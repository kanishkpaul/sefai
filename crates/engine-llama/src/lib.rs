//! One local GGUF model, with bounded prefill and synchronous streaming.
//!
//! The caller owns cancellation and output. Contexts are released after each
//! one-shot request; interactive callers may retain a model-bound prefix cache.
//! Weights stay loaded until the model is dropped. Native model files are trusted inputs, not sandboxed data.

mod error;
mod limits;
mod text;

pub use error::Error;
pub use limits::{PROMPT_BYTES_MAX, Settings};

use llama_cpp_2::{
    LlamaBackendDeviceType,
    context::params::LlamaContextParams,
    list_llama_ggml_backend_devices,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{
        LlamaChatMessage, LlamaModel,
        params::{LlamaModelParams, LlamaSplitMode},
    },
    sampling::LlamaSampler,
    token::LlamaToken,
};
use sefai_core::Prompt;
use std::{
    cell::RefCell,
    fs::File,
    io::{self, Read},
    num::NonZeroU32,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const MODEL_BYTES_MAX: u64 = 1024 * 1024 * 1024 * 1024;
const MESSAGES_MAX: usize = 1024;
const TOKEN_PIECE_BYTES_MAX: usize = 1024 * 1024;

pub struct Engine {
    backend: LlamaBackend,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Device {
    #[default]
    Cpu,
    /// A ggml device index and backend name; indices come from `Engine::devices`.
    Gpu { backend: GpuBackend, index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuBackend {
    Metal,
    Cuda,
    Vulkan,
}

impl GpuBackend {
    fn name(self) -> &'static str {
        match self {
            Self::Metal => "MTL",
            Self::Cuda => "CUDA",
            Self::Vulkan => "Vulkan",
        }
    }
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub index: usize,
    pub backend: String,
    pub name: String,
}

pub struct Model<'engine> {
    model: LlamaModel,
    engine: &'engine Engine,
    generating: RefCell<()>,
    device: Device,
    piece_bytes_max: usize,
}

/// A model-bound context whose reusable prefix is verified against rendered tokens.
/// Dropping or clearing it frees the context; no state is serialized.
#[derive(Default)]
pub struct Cache<'model> {
    context: Option<llama_cpp_2::context::LlamaContext<'model>>,
    tokens: Vec<LlamaToken>,
    model: Option<&'model LlamaModel>,
    settings: Option<Settings>,
    reused_tokens: usize,
}

impl Cache<'_> {
    pub fn clear(&mut self) {
        self.context = None;
        self.tokens = Vec::new();
        self.model = None;
        self.settings = None;
        self.reused_tokens = 0;
    }

    pub fn reused_tokens(&self) -> usize {
        self.reused_tokens
    }
}

/// Tokens belong to the model that produced them and cannot outlive it.
pub struct Input<'model> {
    tokens: Vec<LlamaToken>,
    model: &'model LlamaModel,
}

impl Input<'_> {
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    EndOfGeneration,
    OutputLimit,
    Cancelled,
}

#[derive(Debug)]
pub struct Statistics {
    pub prompt_tokens: usize,
    /// Includes a sampled EOG token when generation ends with one.
    pub generated_tokens: u32,
    pub prefill: Duration,
    /// Includes sampling and the caller's output callback, excluding prefill.
    pub generation: Duration,
    pub stop: Stop,
}

impl Engine {
    /// Initializes the process-wide backend; only one engine may exist at a time.
    ///
    /// # Errors
    /// Returns an error if another engine already owns the backend.
    pub fn new() -> Result<Self, Error> {
        if !cfg!(feature = "native-logs") {
            static LOGGING: std::sync::Once = std::sync::Once::new();
            LOGGING.call_once(|| {
                llama_cpp_2::send_logs_to_tracing(
                    llama_cpp_2::LogOptions::default().with_logs_enabled(false),
                )
            });
        }
        Ok(Self {
            backend: LlamaBackend::init()
                .map_err(|error| Error::operation("initializing llama.cpp", error))?,
        })
    }

    pub fn devices(&self) -> Vec<DeviceInfo> {
        list_llama_ggml_backend_devices()
            .into_iter()
            .map(|device| DeviceInfo {
                index: device.index,
                backend: device.backend,
                name: device.description,
            })
            .collect()
    }

    /// Loads a GGUF v3 file without changing devices or downloading anything.
    ///
    /// CPU mode disables model, KV and operation offload. Linked backends may
    /// still initialize drivers and retain process-wide allocation caches.
    /// The `native-logs` build feature exposes unsanitized native stderr for
    /// placement diagnosis; ordinary builds suppress native logging.
    ///
    /// # Errors
    /// Returns an error for an invalid file, unavailable device, cancelled load,
    /// unsupported architecture, or native loading failure.
    pub fn load(
        &mut self,
        path: &Path,
        device: Device,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Model<'_>, Error> {
        validate_model_file(path)?;
        check_cancelled(&cancelled)?;
        let mut params = model_params(device)?;
        let loading_cancelled = cancelled.clone();
        params = params.with_progress_callback(move |_| !loading_cancelled.load(Ordering::Relaxed));
        let model = LlamaModel::load_from_file(&self.backend, path, &params).map_err(|error| {
            Error::Invalid(format!("loading model {}: {error}", path.display()))
        })?;
        check_cancelled(&cancelled)?;
        let architecture = model
            .meta_val_str("general.architecture")
            .map_err(|error| Error::operation("reading model architecture", error))?;
        if !matches!(
            architecture.as_str(),
            "llama"
                | "qwen2"
                | "qwen3"
                | "gemma"
                | "gemma2"
                | "gemma3"
                | "phi3"
                | "gpt2"
                | "starcoder2"
        ) {
            return Err(Error::Invalid(format!(
                "unsupported decoder architecture: {architecture}"
            )));
        }
        let vocab = model.vocab();
        let piece_bytes_max = vocab
            .tokens()
            .filter_map(|token| vocab.text(token))
            .map(|text| text.to_bytes().len())
            .max()
            .unwrap_or(0);
        if piece_bytes_max == 0 || piece_bytes_max > TOKEN_PIECE_BYTES_MAX {
            return Err(Error::Invalid(
                "invalid or oversized vocabulary token".into(),
            ));
        }
        Ok(Model {
            model,
            engine: self,
            generating: RefCell::new(()),
            device,
            piece_bytes_max,
        })
    }
}

impl Model<'_> {
    pub fn context_tokens_max(&self) -> u32 {
        self.model.n_ctx_train()
    }

    pub fn device(&self) -> Device {
        self.device
    }

    /// Uses the GGUF's template and merges system sections before conversation.
    ///
    /// # Errors
    /// Rejects blank/oversize prompts and missing or unsupported native templates.
    pub fn prepare(&self, prompt: &Prompt) -> Result<Input<'_>, Error> {
        let bytes = prompt
            .conversation()
            .iter()
            .map(|message| message.content.len())
            .chain(prompt.sections().iter().map(|section| section.text.len()))
            .try_fold(0usize, |total, length| total.checked_add(length));
        if bytes.is_none_or(|bytes| bytes > PROMPT_BYTES_MAX)
            || prompt.conversation().len() + prompt.sections().len() > MESSAGES_MAX
            || !prompt
                .conversation()
                .iter()
                .any(|message| !message.content.trim().is_empty())
        {
            return Err(Error::Invalid(format!(
                "prompt must contain text within {PROMPT_BYTES_MAX} bytes and {MESSAGES_MAX} messages/sections"
            )));
        }
        let messages = prompt
            .to_messages()
            .into_iter()
            .map(|message| {
                LlamaChatMessage::new(message.role.as_str().into(), message.content)
                    .map_err(|error| Error::operation("preparing chat message", error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let template = self.model.chat_template(None).map_err(|error| {
            Error::operation(
                "reading chat template; use explicit raw mode for an untemplated model",
                error,
            )
        })?;
        let rendered = self
            .model
            .apply_chat_template(&template, &messages, true)
            .map_err(|error| Error::operation("applying model chat template", error))?;
        validate_text_and_tokenize(&self.model, &rendered, true)
    }

    /// Tokenizes literal text without interpreting role markers as special tokens.
    ///
    /// # Errors
    /// Rejects blank input, nul bytes, and input above `PROMPT_BYTES_MAX`.
    pub fn prepare_raw(&self, text: &str) -> Result<Input<'_>, Error> {
        validate_text_and_tokenize(&self.model, text, false)
    }

    /// Streams complete UTF-8; malformed bytes and unfinished final characters
    /// become U+FFFD. The callback owns terminal sanitization and backpressure.
    /// Cancellation is checked between native calls, not inside a decode call.
    /// Zero output validates the request but performs no decode or allocation.
    ///
    /// # Errors
    /// Rejects mismatched input/models and invalid limits before decoding.
    /// Native failures and callback I/O errors abort the request.
    pub fn generate(
        &self,
        input: &Input<'_>,
        settings: Settings,
        cancelled: impl FnMut() -> bool,
        emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics, Error> {
        self.generate_cached(&mut Cache::default(), input, settings, cancelled, emit)
    }

    /// Reuses only equal rendered prompt tokens for this model and settings.
    /// Replays the final prompt token to obtain fresh logits, and invalidates
    /// the context on cancellation or any error. Settings changes free it first.
    ///
    /// # Errors
    /// Returns the same validation, native and callback errors as `generate`.
    pub fn generate_cached<'model>(
        &'model self,
        cache: &mut Cache<'model>,
        input: &Input<'_>,
        settings: Settings,
        cancelled: impl FnMut() -> bool,
        emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics, Error> {
        let result = self.generate_inner(cache, input, settings, cancelled, emit, || {
            LlamaSampler::chain_simple([
                LlamaSampler::penalties(self.model.vocab().n_tokens(), 64, 1.0, 0.1, 0.0),
                LlamaSampler::greedy(),
            ])
        });
        if result.as_ref().is_err() || result.as_ref().is_ok_and(|s| s.stop == Stop::Cancelled) {
            cache.clear();
        }
        result
    }

    fn generate_inner<'model>(
        &'model self,
        cache: &mut Cache<'model>,
        input: &Input<'_>,
        settings: Settings,
        mut cancelled: impl FnMut() -> bool,
        mut emit: impl FnMut(&str) -> io::Result<()>,
        create_sampler: impl FnOnce() -> LlamaSampler,
    ) -> Result<Statistics, Error> {
        let _generation = self
            .generating
            .try_borrow_mut()
            .map_err(|_| Error::Invalid("model is already generating".into()))?;
        if !std::ptr::eq(input.model, &self.model) {
            return Err(Error::Invalid("input belongs to a different model".into()));
        }
        settings.validate(input.tokens.len(), self.model.n_ctx_train())?;
        let mut statistics = Statistics {
            prompt_tokens: input.tokens.len(),
            generated_tokens: 0,
            prefill: Duration::ZERO,
            generation: Duration::ZERO,
            stop: Stop::OutputLimit,
        };
        if cancelled() {
            statistics.stop = Stop::Cancelled;
            return Ok(statistics);
        }
        if settings.output_tokens == 0 {
            return Ok(statistics);
        }
        if cache.settings != Some(settings) {
            cache.clear();
        }
        if cache
            .model
            .is_some_and(|model| !std::ptr::eq(model, &self.model))
        {
            cache.clear();
        }
        if cache.context.is_none() {
            cache.context = Some(
                self.model
                    .new_context(&self.engine.backend, context_params(settings, self.device))
                    .map_err(|error| Error::operation("allocating model context", error))?,
            );
            cache.settings = Some(settings);
            cache.model = Some(&self.model);
        }
        let context = cache.context.as_mut().expect("context was allocated above");
        let mut prefix = reusable_prefix(&cache.tokens, &input.tokens);
        if !context
            .clear_kv_cache_seq(Some(0), Some(prefix as u32), None)
            .map_err(|error| Error::operation("invalidating cache suffix", error))?
        {
            context.clear_kv_cache();
            prefix = 0;
        }
        cache.reused_tokens = prefix;
        let mut batch = LlamaBatch::new(settings.batch_tokens as usize, 1);
        let prefill_start = Instant::now();
        for (chunk_index, chunk) in input.tokens[prefix..]
            .chunks(settings.batch_tokens as usize)
            .enumerate()
        {
            if cancelled() {
                statistics.stop = Stop::Cancelled;
                statistics.prefill = prefill_start.elapsed();
                return Ok(statistics);
            }
            batch.clear();
            let offset = prefix + chunk_index * settings.batch_tokens as usize;
            for (index, token) in chunk.iter().enumerate() {
                let position = offset + index;
                batch
                    .add(
                        *token,
                        checked_position(position)?,
                        &[0],
                        position + 1 == input.tokens.len(),
                    )
                    .map_err(|error| Error::operation("building prefill batch", error))?;
            }
            context
                .decode(&mut batch)
                .map_err(|error| Error::operation("decoding prompt", error))?;
        }
        context.synchronize();
        statistics.prefill = prefill_start.elapsed();
        cache.tokens.clone_from(&input.tokens);
        let mut sampler = create_sampler();
        sampler.accept_many(&input.tokens);
        let mut piece = Vec::with_capacity(self.piece_bytes_max);
        let mut text = text::Text::new(self.piece_bytes_max);
        let generation_start = Instant::now();
        for index in 0..settings.output_tokens {
            if cancelled() {
                statistics.stop = Stop::Cancelled;
                break;
            }
            let token = sample(&mut sampler, context);
            statistics.generated_tokens += 1;
            if self.model.vocab().is_eog(token) {
                statistics.stop = Stop::EndOfGeneration;
                break;
            }
            piece.clear();
            self.model
                .vocab()
                .token_to_piece_into(token, &mut piece, false, None);
            text.push(&piece, &mut emit)?;
            if cancelled() {
                statistics.stop = Stop::Cancelled;
                break;
            }
            if index + 1 == settings.output_tokens {
                break;
            }
            batch.clear();
            batch
                .add(
                    token,
                    checked_position(input.tokens.len() + index as usize)?,
                    &[0],
                    true,
                )
                .map_err(|error| Error::operation("building generation batch", error))?;
            context
                .decode(&mut batch)
                .map_err(|error| Error::operation("decoding output", error))?;
        }
        text.finish(&mut emit)?;
        statistics.generation = generation_start.elapsed();
        Ok(statistics)
    }
}

fn model_params(device: Device) -> Result<LlamaModelParams, Error> {
    let params = LlamaModelParams::default()
        .with_split_mode(LlamaSplitMode::None)
        .with_main_gpu(0)
        .with_use_mmap(true);
    match device {
        Device::Cpu => params
            .with_n_gpu_layers(0)
            .with_devices(&[])
            .map_err(|error| Error::operation("selecting cpu", error)),
        Device::Gpu { backend, index } => {
            let found = list_llama_ggml_backend_devices().into_iter().any(|device| {
                device.index == index
                    && device.backend == backend.name()
                    && matches!(
                        device.device_type,
                        LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::IntegratedGpu
                    )
            });
            if !found {
                return Err(Error::Invalid(format!(
                    "{} gpu device {index} is unavailable; list devices first",
                    backend.name()
                )));
            }
            params
                .with_n_gpu_layers(i32::MAX as u32)
                .with_devices(&[index])
                .map_err(|error| Error::operation("selecting gpu", error))
        }
    }
}

fn context_params(settings: Settings, device: Device) -> LlamaContextParams {
    let offload = device != Device::Cpu;
    LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(settings.context_tokens))
        .with_n_batch(settings.batch_tokens)
        .with_n_ubatch(settings.microbatch_tokens)
        .with_n_seq_max(1)
        .with_n_threads(settings.threads as i32)
        .with_n_threads_batch(settings.threads as i32)
        .with_offload_kqv(offload)
        .with_op_offload(offload)
}

fn sample(
    sampler: &mut LlamaSampler,
    context: &llama_cpp_2::context::LlamaContext<'_>,
) -> LlamaToken {
    // Sampling already accepts the selected token into every stateful sampler.
    sampler.sample(context, -1)
}

fn checked_position(position: usize) -> Result<i32, Error> {
    i32::try_from(position).map_err(|_| Error::Invalid("token position exceeds i32".into()))
}

fn validate_text_and_tokenize<'model>(
    model: &'model LlamaModel,
    text: &str,
    chat: bool,
) -> Result<Input<'model>, Error> {
    limits::validate_text(text)?;
    let vocab = model.vocab();
    let mut tokens = vocab.tokenize(text.as_bytes(), false, chat);
    if vocab.should_add_bos() && tokens.first() != Some(&vocab.bos()) {
        tokens.insert(0, vocab.bos());
    }
    if !chat && vocab.should_add_eos() && tokens.last() != Some(&vocab.eos()) {
        tokens.push(vocab.eos());
    }
    if tokens.is_empty() {
        return Err(Error::Invalid("tokenizer produced no prompt tokens".into()));
    }
    Ok(Input { tokens, model })
}

fn validate_model_file(path: &Path) -> Result<(), Error> {
    let mut file = File::open(path)
        .map_err(|error| Error::Invalid(format!("opening model {}: {error}", path.display())))?;
    let metadata = file
        .metadata()
        .map_err(|error| Error::operation("reading model file metadata", error))?;
    if !metadata.is_file() || !(24..=MODEL_BYTES_MAX).contains(&metadata.len()) {
        return Err(Error::Invalid(
            "model must be a regular file between 24 bytes and 1 tib".into(),
        ));
    }
    let mut header = [0; 8];
    file.read_exact(&mut header)
        .map_err(|error| Error::operation("reading gguf header", error))?;
    if &header[..4] != b"GGUF"
        || u32::from_le_bytes(header[4..].try_into().expect("four version bytes")) != 3
    {
        return Err(Error::Invalid("model must use gguf version 3".into()));
    }
    Ok(())
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_disables_cache_and_operation_offload() {
        let params = context_params(Settings::default(), Device::Cpu);
        assert!(!params.offload_kqv());
        assert!(!params.op_offload());
    }

    #[test]
    fn cpu_selects_no_gpu_devices_or_layers() {
        let params = model_params(Device::Cpu).expect("cpu params");
        assert!(params.devices().is_empty());
        assert_eq!(params.n_gpu_layers(), 0);
        assert_eq!(
            params.split_mode().expect("split mode"),
            LlamaSplitMode::None
        );
    }

    #[test]
    fn missing_model_is_an_error() {
        let path = std::env::temp_dir().join(format!("sefai-missing-{}", std::process::id()));
        assert!(validate_model_file(&path).is_err());
    }

    #[test]
    fn wrong_model_magic_is_rejected() {
        let path =
            std::env::temp_dir().join(format!("sefai-invalid-header-{}.gguf", std::process::id()));
        let mut file = File::create_new(&path).expect("unique fixture path");
        std::io::Write::write_all(&mut file, &[0; 24]).expect("fixture contents");
        let result = validate_model_file(&path);
        drop(file);
        std::fs::remove_file(path).expect("remove fixture");
        assert!(result.is_err());
    }

    #[test]
    fn directory_is_not_a_model() {
        assert!(validate_model_file(&std::env::temp_dir()).is_err());
    }

    #[test]
    fn excessive_position_is_rejected() {
        assert!(checked_position(i32::MAX as usize + 1).is_err());
    }
}

#[cfg(all(test, feature = "model-tests"))]
mod model_tests;

fn reusable_prefix<T: PartialEq>(cached: &[T], next: &[T]) -> usize {
    cached
        .iter()
        .zip(next)
        .take_while(|(left, right)| left == right)
        .count()
        .min(next.len().saturating_sub(1))
}

#[cfg(test)]
mod cache_tests {
    use super::reusable_prefix;

    #[test]
    fn changed_instructions_invalidate_the_suffix() {
        assert_eq!(reusable_prefix(&[1, 2, 3, 4], &[1, 9, 3, 4]), 1);
    }

    #[test]
    fn identical_prompt_replays_its_last_token_for_logits() {
        assert_eq!(reusable_prefix(&[1, 2, 3], &[1, 2, 3]), 2);
    }

    #[test]
    fn empty_cache_reuses_nothing() {
        assert_eq!(reusable_prefix::<u32>(&[], &[1]), 0);
    }
}
