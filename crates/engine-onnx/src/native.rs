use crate::{
    Bundle,
    bundle::{json, read},
    signature::{self, Element, Layout, Port},
};
use ort::{
    environment::Environment,
    logging::LogLevel,
    session::{Session, SessionInputValue, builder::GraphOptimizationLevel},
    value::{DynValue, Tensor, TensorElementType},
};
use sefai_core::{Prompt, Role, Settings, Statistics, Stop};
use std::{
    borrow::Cow,
    cell::RefCell,
    io,
    path::Path,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Error>;

const OUTPUT_BYTES_MAX: usize = 1024 * 1024;
const PROMPT_TOKENS_MAX: usize = 131_072;
// Matches the GGUF sampler: greedy with a 0.1 frequency penalty over the last 64 tokens.
const PENALTY_TOKENS: usize = 64;
const FREQUENCY_PENALTY: f32 = 0.1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("onnx file or output: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Limits(#[from] sefai_core::LimitsError),
    #[error("onnx runtime: {0}")]
    Runtime(String),
    #[error("onnx tokenization: {0}")]
    Tokenizer(String),
    #[error("onnx chat template: {0}")]
    Template(#[from] minijinja::Error),
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}

impl<R> From<ort::Error<R>> for Error {
    fn from(error: ort::Error<R>) -> Self {
        Self::Runtime(error.message().to_owned())
    }
}

pub struct Model {
    session: RefCell<Session>,
    tokenizer: tokenizers::Tokenizer,
    chat: Chat,
    bundle: Bundle,
    layout: Layout,
    threads: u32,
}

/// Retains the graph's present tensors for one model and cannot outlive that model.
///
/// ```compile_fail
/// use sefai_engine_onnx::{Cache, Model};
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
    /// Alternating key and value tensors per layer, covering every token in `tokens`.
    past: Vec<DynValue>,
    tokens: Vec<u32>,
    reused: usize,
}

pub struct Input<'model> {
    tokens: Vec<u32>,
    model: &'model Model,
}

impl Input<'_> {
    /// Yields model-native IDs for reproducible tokenization comparisons.
    pub fn token_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.tokens.iter().copied()
    }
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

impl Cache<'_> {
    pub fn clear(&mut self) {
        self.model = None;
        self.past.clear();
        self.tokens.clear();
        self.reused = 0;
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
    /// Loads a validated decoder graph on the CPU execution provider.
    /// ONNX Runtime binds intra-op `threads` to the session, so every request must use the same count.
    /// Cancellation is checked around native loading, not inside it.
    pub fn load(path: &Path, threads: u32, cancelled: Arc<AtomicBool>) -> Result<Self> {
        if !(1..=sefai_core::THREADS_MAX).contains(&threads) {
            return Err(Error::Invalid(format!(
                "threads must be 1..={}",
                sefai_core::THREADS_MAX
            )));
        }
        let bundle = super::validate(path)?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("model loading cancelled".into());
        }
        let tokenizer = tokenizers::Tokenizer::from_bytes(read(&bundle.root, "tokenizer.json")?)
            .map_err(|error| Error::Tokenizer(error.to_string()))?;
        let config = json(&bundle.root, "tokenizer_config.json")?;
        let chat = Chat::new(&config)?;
        let session = Session::builder(environment()?)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(threads as usize)?
            .commit_from_file(&bundle.graph)?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("model loading cancelled".into());
        }
        let inputs = session.inputs().iter().map(port).collect::<Vec<_>>();
        let outputs = session.outputs().iter().map(port).collect::<Vec<_>>();
        let layout =
            signature::check(&ports(&inputs), &ports(&outputs), &bundle).map_err(Error::Invalid)?;
        Ok(Self {
            session: RefCell::new(session),
            tokenizer,
            chat,
            bundle,
            layout,
            threads,
        })
    }

    pub fn context_tokens_max(&self) -> u32 {
        self.bundle.context
    }

    /// Returns the intra-op thread count every request must use.
    pub fn threads(&self) -> u32 {
        self.threads
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
        let template = self
            .chat
            .environment
            .as_ref()
            .ok_or("onnx tokenizer_config.json has no chat template")?;
        let messages = prompt
            .to_messages()
            .into_iter()
            .map(|message| {
                minijinja::context! {
                    role => match message.role {
                        Role::System => "system",
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::Tool => "tool",
                    },
                    content => message.content,
                }
            })
            .collect::<Vec<_>>();
        let rendered = template.get_template("chat")?.render(minijinja::context! {
            messages => messages,
            add_generation_prompt => true,
            enable_thinking => false,
            ..minijinja::Value::from_serialize(&self.chat.special)
        })?;
        let special = !self
            .chat
            .bos
            .as_ref()
            .is_some_and(|bos| rendered.starts_with(bos.as_str()));
        self.encode(&rendered, special)
    }

    pub fn prepare_raw(&self, text: &str) -> Result<Input<'_>> {
        self.encode(text, false)
    }

    fn encode(&self, text: &str, special: bool) -> Result<Input<'_>> {
        if text.trim().is_empty()
            || text.len() > sefai_core::PROMPT_BYTES_MAX
            || text.contains('\0')
        {
            return Err(
                "prompt must contain text within 1048576 bytes without nul characters".into(),
            );
        }
        let tokens = self
            .tokenizer
            .encode(text, special)
            .map_err(|error| Error::Tokenizer(error.to_string()))?
            .get_ids()
            .to_vec();
        if tokens.is_empty() || tokens.len() > PROMPT_TOKENS_MAX {
            return Err("onnx prompt must contain 1..131072 tokens".into());
        }
        if tokens
            .iter()
            .any(|token| *token as usize >= self.bundle.vocab)
        {
            return Err("onnx tokenizer produced an id outside the graph vocabulary".into());
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
        settings.validate(input.token_count(), self.bundle.context)?;
        if settings.threads != self.threads {
            return Err(Error::Invalid(format!(
                "onnx uses {} threads fixed when the model loaded",
                self.threads
            )));
        }
        let mut statistics = Statistics {
            prompt_tokens: input.token_count(),
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
        let mut session = self
            .session
            .try_borrow_mut()
            .map_err(|_| "model is already generating")?;
        // Past tensors depend only on the tokens; a strict prefix leaves one token to produce logits.
        if cache.model.is_some_and(|model| !std::ptr::eq(model, self))
            || cache.tokens.len() >= input.tokens.len()
            || !input.tokens.starts_with(&cache.tokens)
        {
            cache.clear();
        }
        cache.model = Some(self);
        cache.reused = cache.tokens.len();
        let mut history = input.tokens.clone();
        let started = Instant::now();
        let mut logits = Vec::new();
        for chunk in input.tokens[cache.tokens.len()..].chunks(settings.microbatch_tokens as usize)
        {
            if cancelled() {
                statistics.stop = Stop::Cancelled;
                statistics.prefill = started.elapsed();
                return Ok(statistics);
            }
            logits = self.step(&mut session, cache, chunk)?;
        }
        statistics.prefill = started.elapsed();
        let started = Instant::now();
        let mut decoder = self.tokenizer.decode_stream(true);
        let mut output_bytes = 0usize;
        while statistics.generated_tokens < settings.output_tokens {
            if cancelled() {
                statistics.stop = Stop::Cancelled;
                break;
            }
            let token = select(&mut logits, &history);
            statistics.generated_tokens += 1;
            if self.bundle.eos.binary_search(&token).is_ok() {
                statistics.stop = Stop::EndOfGeneration;
                break;
            }
            if let Some(text) = decoder
                .step(token)
                .map_err(|error| Error::Tokenizer(error.to_string()))?
            {
                output_bytes = output_bytes
                    .checked_add(text.len())
                    .filter(|bytes| *bytes <= OUTPUT_BYTES_MAX)
                    .ok_or("onnx output exceeds 1048576 bytes")?;
                emit(&text)?;
            }
            history.push(token);
            if statistics.generated_tokens < settings.output_tokens {
                logits = self.step(&mut session, cache, &[token])?;
            }
        }
        statistics.generation = started.elapsed();
        Ok(statistics)
    }

    /// Runs one chunk, replaces the cache with the graph's present tensors and returns the last row of logits.
    fn step(
        &self,
        session: &mut Session,
        cache: &mut Cache<'_>,
        tokens: &[u32],
    ) -> Result<Vec<f32>> {
        let past = cache.tokens.len();
        let total = past + tokens.len();
        let ids = tokens
            .iter()
            .map(|token| i64::from(*token))
            .collect::<Vec<_>>();
        let mut inputs: Vec<(Cow<'static, str>, SessionInputValue<'_>)> = vec![
            (
                "input_ids".into(),
                Tensor::from_array(([1, tokens.len()], ids))?.into(),
            ),
            (
                "attention_mask".into(),
                Tensor::from_array(([1, total], vec![1i64; total]))?.into(),
            ),
        ];
        if self.layout.position_ids {
            let positions = (past as i64..total as i64).collect::<Vec<_>>();
            inputs.push((
                "position_ids".into(),
                Tensor::from_array(([1, tokens.len()], positions))?.into(),
            ));
        }
        let shape = [1, self.bundle.kv_heads, 0, self.bundle.head_dim];
        let mut previous = std::mem::take(&mut cache.past).into_iter();
        for layer in 0..self.bundle.layers {
            for part in ["key", "value"] {
                let value = match previous.next() {
                    Some(value) => SessionInputValue::from(value),
                    None => Tensor::from_array((shape, Vec::<f32>::new()))?.into(),
                };
                inputs.push((format!("past_key_values.{layer}.{part}").into(), value));
            }
        }
        let mut outputs = session.run(inputs)?;
        let mut present = Vec::with_capacity(self.bundle.layers * 2);
        for layer in 0..self.bundle.layers {
            for part in ["key", "value"] {
                present.push(
                    outputs
                        .remove(format!("present.{layer}.{part}"))
                        .ok_or("onnx graph omitted a present tensor")?,
                );
            }
        }
        let (shape, values) = outputs["logits"].try_extract_tensor::<f32>()?;
        if shape.len() != 3
            || shape[1] != tokens.len() as i64
            || shape[2] != self.bundle.vocab as i64
        {
            return Err("onnx logits shape differs from the admitted signature".into());
        }
        let row = values[values.len() - self.bundle.vocab..].to_vec();
        cache.past = present;
        cache.tokens.extend_from_slice(tokens);
        Ok(row)
    }
}

fn select(logits: &mut [f32], history: &[u32]) -> u32 {
    let recent = &history[history.len().saturating_sub(PENALTY_TOKENS)..];
    for token in recent {
        if let Some(logit) = logits.get_mut(*token as usize) {
            *logit -= FREQUENCY_PENALTY;
        }
    }
    logits
        .iter()
        .enumerate()
        .fold((0, f32::NEG_INFINITY), |best, (token, logit)| {
            if *logit > best.1 {
                (token, *logit)
            } else {
                best
            }
        })
        .0 as u32
}

type Declared = (String, Element, Vec<i64>);

fn port(outlet: &ort::value::Outlet) -> Declared {
    let dtype = outlet.dtype();
    let element = match dtype.tensor_type() {
        Some(TensorElementType::Int64) => Element::I64,
        Some(TensorElementType::Float32) => Element::F32,
        Some(TensorElementType::Float16) => Element::F16,
        _ => Element::Other,
    };
    let shape = dtype
        .tensor_shape()
        .map_or_else(Vec::new, |shape| shape.to_vec());
    (outlet.name().to_owned(), element, shape)
}

fn ports(declared: &[Declared]) -> Vec<Port<'_>> {
    declared
        .iter()
        .map(|(name, element, shape)| Port {
            name,
            element: *element,
            shape,
        })
        .collect()
}

/// The tokenizer's compiled chat template; raw prompts remain usable without one.
struct Chat {
    environment: Option<minijinja::Environment<'static>>,
    special: serde_json::Map<String, serde_json::Value>,
    bos: Option<String>,
}

impl Chat {
    fn new(config: &serde_json::Value) -> Result<Self> {
        let content = |value: &serde_json::Value| {
            value
                .as_str()
                .or_else(|| value.get("content").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
        };
        let mut special = serde_json::Map::new();
        for name in ["bos_token", "eos_token", "pad_token", "unk_token"] {
            if let Some(token) = content(&config[name]) {
                special.insert(name.into(), token.into());
            }
        }
        let bos = content(&config["bos_token"]);
        let environment = match &config["chat_template"] {
            serde_json::Value::Null => None,
            serde_json::Value::String(source) => {
                let mut environment = minijinja::Environment::new();
                environment.set_trim_blocks(true);
                environment.set_lstrip_blocks(true);
                environment.set_unknown_method_callback(
                    minijinja_contrib::pycompat::unknown_method_callback,
                );
                environment.add_function(
                    "raise_exception",
                    |message: String| -> std::result::Result<String, minijinja::Error> {
                        Err(minijinja::Error::new(
                            minijinja::ErrorKind::InvalidOperation,
                            message,
                        ))
                    },
                );
                environment.add_template_owned("chat", source.clone())?;
                Some(environment)
            }
            _ => return Err("onnx chat_template must be one template string".into()),
        };
        Ok(Self {
            environment,
            special,
            bos,
        })
    }
}

/// ONNX Runtime permits one environment per process, so it outlives every model.
fn environment() -> Result<&'static Environment> {
    static ENVIRONMENT: OnceLock<std::result::Result<Environment, String>> = OnceLock::new();
    ENVIRONMENT
        .get_or_init(|| {
            let environment = ort::environment::init()
                .with_name("sefai")
                .with_telemetry(false)
                .build()
                .map_err(|error| error.message().to_owned())?;
            // Failures reach callers as errors; native log lines would corrupt the terminal.
            environment.set_log_level(LogLevel::Fatal);
            Ok(environment)
        })
        .as_ref()
        .map_err(|error| Error::Runtime(error.clone()))
}
