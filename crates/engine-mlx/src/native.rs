use mlx_lm::{
    ChatContinuation, ChatTemplateOptions, FinishReason, GenerationEvent, GenerationOptions,
    TokenId,
};
use sefai_core::{Prompt, Role, Settings, Statistics, Stop};
use std::{
    cell::RefCell,
    io,
    num::NonZeroUsize,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("mlx file or output: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Limits(#[from] sefai_core::LimitsError),
    #[error("mlx native operation: {0}")]
    Native(#[from] mlx_rs::error::Exception),
    #[error("mlx loading bundle: {0}")]
    Load(#[from] mlx_lm::LoadError),
    #[error("mlx tokenization: {0}")]
    Tokenizer(#[from] mlx_lm::TokenizerError),
    #[error("mlx chat template: {0}")]
    Template(#[from] mlx_lm::ChatTemplateError),
    #[error("mlx cache: {0}")]
    Cache(#[from] mlx_lm::CacheError),
    #[error("mlx generation: {0}")]
    Generation(#[from] mlx_lm::GenerationError),
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}

pub struct Model {
    model: RefCell<mlx_lm::Model>,
    context: u32,
    _lease: Lease,
}

/// Retains native KV tensors for one model and cannot outlive that model.
///
/// ```compile_fail
/// use sefai_engine_mlx::{Cache, Model};
/// use sefai_core::Settings;
/// fn invalid(model: Model) -> Result<(), Box<dyn std::error::Error>> {
///     let input = model.prepare_raw("hello")?;
///     let mut cache = Cache::default();
///     model.generate_cached(&mut cache, &input, Settings::default(), || false, |_| Ok(()))?;
///     drop(input);
///     drop(model);
///     Ok(())
/// }
/// ```
#[derive(Default)]
pub struct Cache<'model> {
    model: Option<&'model Model>,
    cache: Option<mlx_lm::Cache>,
    reused: usize,
    settings: Option<Settings>,
}

pub struct Input<'model> {
    tokens: Vec<TokenId>,
    model: &'model Model,
}

impl Input<'_> {
    /// Yields model-native IDs for reproducible tokenization comparisons.
    pub fn token_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.tokens.iter().copied().map(u32::from)
    }
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

impl Cache<'_> {
    pub fn clear(&mut self) {
        self.cache = None;
        self.model = None;
        self.reused = 0;
        self.settings = None;
    }
    pub fn reused_tokens(&self) -> usize {
        self.reused
    }
}

impl Drop for Cache<'_> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Model {
    /// Loads a validated local bundle onto the worker's default Metal device.
    /// Cancellation is checked around native loading, not inside it.
    pub fn load(path: &Path, cancelled: Arc<AtomicBool>) -> Result<Self> {
        let context = super::validate(path)?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("model loading cancelled".into());
        }
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("an mlx model is already loaded".into());
        }
        let lease = Lease;
        mlx_rs::Device::set_default(&mlx_rs::Device::gpu());
        // Limit scratch retention independently of the live KV tensors.
        mlx_rs::memory::set_cache_limit(64 * 1024 * 1024)?;
        let model = mlx_lm::Model::from_dir(path)?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("model loading cancelled".into());
        }
        Ok(Self {
            model: RefCell::new(model),
            context,
            _lease: lease,
        })
    }

    pub fn context_tokens_max(&self) -> u32 {
        self.context
    }

    pub fn prepare(&self, prompt: &Prompt) -> Result<Input<'_>> {
        let bytes = prompt
            .conversation()
            .iter()
            .map(|message| message.content.len())
            .chain(prompt.sections().iter().map(|section| section.text.len()))
            .try_fold(0usize, |total, length| total.checked_add(length));
        if prompt
            .conversation()
            .len()
            .saturating_add(prompt.sections().len())
            > 1024
            || bytes.is_none_or(|bytes| bytes > sefai_core::PROMPT_BYTES_MAX)
        {
            return Err("prompt exceeds 1024 messages/sections or 1048576 bytes".into());
        }
        let messages = prompt.to_messages();
        let messages = messages
            .into_iter()
            .map(|m| mlx_lm::Message {
                role: match m.role {
                    Role::System => mlx_lm::Role::System,
                    Role::User => mlx_lm::Role::User,
                    Role::Assistant => mlx_lm::Role::Assistant,
                    Role::Tool => mlx_lm::Role::Tool,
                },
                content: m.content,
            })
            .collect::<Vec<_>>();
        let model = self
            .model
            .try_borrow()
            .map_err(|_| "model is already generating")?;
        let rendered = model.tokenizer().render_chat(
            &messages,
            ChatTemplateOptions {
                continuation: ChatContinuation::StartAssistant,
                enable_thinking: Some(false),
            },
        )?;
        let special = !model
            .tokenizer()
            .bos_token()
            .is_some_and(|bos| rendered.starts_with(bos));
        self.encode(&model, &rendered, special)
    }

    pub fn prepare_raw(&self, text: &str) -> Result<Input<'_>> {
        let model = self
            .model
            .try_borrow()
            .map_err(|_| "model is already generating")?;
        self.encode(&model, text, false)
    }

    fn encode(&self, model: &mlx_lm::Model, text: &str, special: bool) -> Result<Input<'_>> {
        if text.trim().is_empty()
            || text.len() > sefai_core::PROMPT_BYTES_MAX
            || text.contains('\0')
        {
            return Err(
                "prompt must contain text within 1048576 bytes without nul characters".into(),
            );
        }
        let tokens = model
            .tokenizer()
            .encode_with_special_tokens(text, special)?;
        if tokens.is_empty() || tokens.len() > 131072 {
            return Err("mlx prompt must contain 1..131072 tokens".into());
        }
        Ok(Input {
            tokens,
            model: self,
        })
    }

    pub fn generate_cached<'model>(
        &'model self,
        cache: &mut Cache<'model>,
        input: &Input<'_>,
        settings: Settings,
        cancelled: impl FnMut() -> bool,
        emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics> {
        let result = self.generate_inner(cache, input, settings, cancelled, emit);
        if result.is_err()
            || result
                .as_ref()
                .is_ok_and(|statistics| statistics.stop == Stop::Cancelled)
        {
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
    ) -> Result<Statistics> {
        if !std::ptr::eq(input.model, self) {
            return Err("input belongs to a different model".into());
        }
        settings.validate(input.token_count(), self.context)?;
        let mut statistics = Statistics {
            prompt_tokens: input.token_count(),
            generated_tokens: 0,
            prefill: Duration::ZERO,
            generation: Duration::ZERO,
            stop: Stop::OutputLimit,
        };
        if cancelled() {
            cache.clear();
            statistics.stop = Stop::Cancelled;
            return Ok(statistics);
        }
        if settings.output_tokens == 0 {
            return Ok(statistics);
        }
        let mut model = self
            .model
            .try_borrow_mut()
            .map_err(|_| "model is already generating")?;
        // Upstream admits exact whole-cache prefixes only; divergent prompts rebuild.
        if cache.model.is_some_and(|model| !std::ptr::eq(model, self))
            || cache.settings != Some(settings)
            || cache.cache.as_ref().is_some_and(|c| {
                c.tokens().len() >= input.tokens.len() || !input.tokens.starts_with(c.tokens())
            })
        {
            cache.clear();
        }
        if cache.cache.is_none() {
            cache.cache = Some(model.new_cache(Default::default())?);
        }
        cache.model = Some(self);
        cache.settings = Some(settings);
        cache.reused = cache
            .cache
            .as_ref()
            .expect("cache was initialized")
            .tokens()
            .len();
        let mut options = GenerationOptions::default();
        options.max_tokens = NonZeroUsize::new(settings.output_tokens as usize)
            .expect("positive output checked above");
        options.prefill_chunk_size =
            NonZeroUsize::new(settings.microbatch_tokens as usize).expect("validated microbatch");
        let started = Instant::now();
        let mut first_token = None;
        let mut output_bytes = 0usize;
        let result = (|| -> Result<()> {
            let generation = model.generate_with_cache(
                mlx_lm::Prompt::Tokens(&input.tokens),
                options,
                cache.cache.as_mut().expect("cache was initialized"),
            )?;
            for event in generation {
                if cancelled() {
                    statistics.stop = Stop::Cancelled;
                    break;
                }
                if let GenerationEvent::Token {
                    text,
                    finish_reason,
                    ..
                } = event?
                {
                    first_token.get_or_insert_with(|| {
                        statistics.prefill = started.elapsed();
                        Instant::now()
                    });
                    statistics.generated_tokens += 1;
                    output_bytes = output_bytes
                        .checked_add(text.len())
                        .filter(|bytes| *bytes <= 1024 * 1024)
                        .ok_or("mlx output exceeds 1048576 bytes")?;
                    emit(&text)?;
                    if finish_reason == Some(FinishReason::Stop) {
                        statistics.stop = Stop::EndOfGeneration;
                    }
                }
                if cancelled() {
                    statistics.stop = Stop::Cancelled;
                    break;
                }
            }
            Ok(())
        })();
        if let Some(started) = first_token {
            statistics.generation = started.elapsed();
        } else {
            statistics.prefill = started.elapsed();
        }
        result?;
        Ok(statistics)
    }
}

static LOADED: AtomicBool = AtomicBool::new(false);
struct Lease;
impl Drop for Lease {
    fn drop(&mut self) {
        // The lease drops after native weights, including on a failed load.
        let _ = mlx_rs::memory::set_cache_limit(0);
        let _ = mlx_rs::memory::clear_cache();
        LOADED.store(false, Ordering::Release);
    }
}

/// Returns process-global MLX active, cached and peak-active byte counts.
/// These exclude host RSS and must not be added to unified-memory RSS.
pub fn memory() -> Result<[usize; 3]> {
    Ok([
        mlx_rs::memory::active_memory()?,
        mlx_rs::memory::cache_memory()?,
        mlx_rs::memory::peak_memory()?,
    ])
}
