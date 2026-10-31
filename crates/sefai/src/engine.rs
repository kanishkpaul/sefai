//! Concrete engine selection belongs to the binary; adapters never depend on each other.
//! Loading admits the artifact against its declared format before any native allocation.

use crate::workspace::Format;
use sefai_core::Prompt;
pub use sefai_core::{PROMPT_BYTES_MAX, Settings, Statistics, Stop};
use std::{
    fmt, io,
    marker::PhantomData,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Device {
    #[default]
    Cpu,
    Gpu {
        backend: GpuBackend,
        index: usize,
    },
    Mlx,
    /// ONNX Runtime fixes the provider and intra-op threads when the session is created.
    Onnx {
        provider: OnnxProvider,
        threads: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnnxProvider {
    Cpu,
    WebGpu,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuBackend {
    Metal,
    Cuda,
    Vulkan,
}

pub struct Engine {
    #[cfg(feature = "gguf")]
    llama: Option<sefai_engine_llama::Engine>,
}

/// Why a model was not loaded, naming the specific blocker.
#[derive(Debug)]
pub enum LoadError {
    /// The selected engine is not compiled into this build or not available on this platform.
    // A build with every engine on Apple Silicon never constructs it.
    #[allow(dead_code)]
    MissingRuntime(&'static str),
    /// The artifact is not the declared format or fails its engine's admission checks.
    UnsupportedArtifact(String),
    /// The native runtime rejected an admitted artifact or device.
    Failed(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRuntime(message) => write!(formatter, "missing runtime: {message}"),
            Self::UnsupportedArtifact(message) => {
                write!(formatter, "unsupported artifact: {message}")
            }
            Self::Failed(message) => write!(formatter, "load failed: {message}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[cfg(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
))]
pub enum Model<'engine> {
    #[cfg(feature = "gguf")]
    Llama(sefai_engine_llama::Model<'engine>),
    #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
    Mlx(Box<sefai_engine_mlx::Model>, PhantomData<&'engine ()>),
    #[cfg(feature = "onnx")]
    Onnx(Box<sefai_engine_onnx::Model>, PhantomData<&'engine ()>),
}

#[cfg(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
))]
pub enum Input<'model> {
    #[cfg(feature = "gguf")]
    Llama(sefai_engine_llama::Input<'model>),
    #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
    Mlx(sefai_engine_mlx::Input<'model>),
    #[cfg(feature = "onnx")]
    Onnx(sefai_engine_onnx::Input<'model>),
}

#[derive(Default)]
pub struct Cache<'model> {
    #[cfg(feature = "gguf")]
    llama: sefai_engine_llama::Cache<'model>,
    #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
    mlx: sefai_engine_mlx::Cache<'model>,
    #[cfg(feature = "onnx")]
    onnx: sefai_engine_onnx::Cache<'model>,
    lifetime: PhantomData<&'model ()>,
}
impl Cache<'_> {
    pub fn clear(&mut self) {
        #[cfg(feature = "gguf")]
        self.llama.clear();
        #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
        self.mlx.clear();
        #[cfg(feature = "onnx")]
        self.onnx.clear();
    }
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "gguf")]
            llama: None,
        })
    }
    /// Admits `path` for the device's engine, then loads it; `format` is the declared format, if any.
    ///
    /// # Errors
    /// Returns the specific blocker: an engine absent from this build, an artifact that is not
    /// the declared format, or a native load failure.
    pub fn load(
        &mut self,
        path: &Path,
        format: Option<Format>,
        device: Device,
        cancelled: Arc<AtomicBool>,
    ) -> std::result::Result<Model<'_>, LoadError> {
        #[cfg(not(all(feature = "mlx", target_os = "macos", target_arch = "aarch64")))]
        let _ = format;
        if let Device::Onnx { provider, threads } = device {
            #[cfg(feature = "onnx")]
            {
                sefai_engine_onnx::validate(path).map_err(unsupported)?;
                return Ok(Model::Onnx(
                    Box::new(
                        sefai_engine_onnx::Model::load(
                            path,
                            match provider {
                                OnnxProvider::Cpu => sefai_engine_onnx::Provider::Cpu,
                                OnnxProvider::WebGpu => sefai_engine_onnx::Provider::WebGpu,
                            },
                            threads,
                            cancelled,
                        )
                        .map_err(failed)?,
                    ),
                    PhantomData,
                ));
            }
            #[cfg(not(feature = "onnx"))]
            {
                let _ = (provider, threads);
                return Err(LoadError::MissingRuntime(
                    "onnx requires the optional onnx feature",
                ));
            }
        }
        if device == Device::Mlx {
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            {
                let bundle = sefai_engine_mlx::validate(path).map_err(unsupported)?;
                admit_mlx_weights(format, bundle.weights)?;
                return Ok(Model::Mlx(
                    Box::new(sefai_engine_mlx::Model::load(path, cancelled).map_err(failed)?),
                    PhantomData,
                ));
            }
            #[cfg(not(all(feature = "mlx", target_os = "macos", target_arch = "aarch64")))]
            return Err(LoadError::MissingRuntime(
                "mlx requires the optional mlx feature on Apple Silicon macos",
            ));
        }
        #[cfg(feature = "gguf")]
        {
            admit_gguf(path)?;
            let device = match device {
                Device::Cpu => sefai_engine_llama::Device::Cpu,
                Device::Gpu { backend, index } => sefai_engine_llama::Device::Gpu {
                    index,
                    backend: match backend {
                        GpuBackend::Metal => sefai_engine_llama::GpuBackend::Metal,
                        GpuBackend::Cuda => sefai_engine_llama::GpuBackend::Cuda,
                        GpuBackend::Vulkan => sefai_engine_llama::GpuBackend::Vulkan,
                    },
                },
                Device::Mlx | Device::Onnx { .. } => unreachable!(),
            };
            if self.llama.is_none() {
                self.llama = Some(sefai_engine_llama::Engine::new().map_err(failed)?);
            }
            Ok(Model::Llama(
                self.llama
                    .as_mut()
                    .expect("engine was initialized")
                    .load(path, device, cancelled)
                    .map_err(failed)?,
            ))
        }
        #[cfg(not(feature = "gguf"))]
        {
            let _ = (path, cancelled);
            Err(LoadError::MissingRuntime(
                "gguf requires the optional gguf feature",
            ))
        }
    }
}

/// GGUF files begin with this magic; anything else would only fail inside llama.cpp.
#[cfg(feature = "gguf")]
fn admit_gguf(path: &Path) -> std::result::Result<(), LoadError> {
    use std::io::Read;
    if path.is_dir() {
        return Err(LoadError::UnsupportedArtifact(
            "gguf model is a directory; select format mlx, gptq, awq or onnx for a bundle".into(),
        ));
    }
    let mut magic = [0; 4];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map_err(|error| LoadError::UnsupportedArtifact(format!("gguf model: {error}")))?;
    if &magic != b"GGUF" {
        return Err(LoadError::UnsupportedArtifact(
            "gguf model must be a file beginning with the GGUF magic".into(),
        ));
    }
    Ok(())
}

/// A declared GPTQ, AWQ or MLX format must match the checkpoint's own quantization metadata.
/// An undeclared format admits whichever layout the MLX bundle validated.
#[cfg(feature = "mlx")]
#[cfg_attr(
    not(all(target_os = "macos", target_arch = "aarch64")),
    allow(dead_code)
)]
fn admit_mlx_weights(
    format: Option<Format>,
    weights: sefai_engine_mlx::Weights,
) -> std::result::Result<(), LoadError> {
    use sefai_engine_mlx::Weights;
    let stored = match weights {
        Weights::Mlx => Format::Mlx,
        Weights::Gptq { .. } => Format::Gptq,
        Weights::Awq { .. } => Format::Awq,
    };
    match format {
        None => Ok(()),
        Some(declared) if declared == stored => Ok(()),
        Some(declared) => Err(LoadError::UnsupportedArtifact(format!(
            "declared format {} but the checkpoint stores {} weights",
            declared.name(),
            stored.name()
        ))),
    }
}

#[cfg(any(
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
))]
fn unsupported(error: io::Error) -> LoadError {
    LoadError::UnsupportedArtifact(error.to_string())
}

fn failed(error: impl fmt::Display) -> LoadError {
    LoadError::Failed(error.to_string())
}

#[cfg(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
))]
impl Input<'_> {
    pub fn token_count(&self) -> usize {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(input) => input.token_count(),
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(input) => input.token_count(),
            #[cfg(feature = "onnx")]
            Self::Onnx(input) => input.token_count(),
        }
    }
}
#[cfg(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
))]
impl Model<'_> {
    pub fn device(&self) -> Device {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(model) => match model.device() {
                sefai_engine_llama::Device::Cpu => Device::Cpu,
                sefai_engine_llama::Device::Gpu { index, backend } => Device::Gpu {
                    index,
                    backend: match backend {
                        sefai_engine_llama::GpuBackend::Metal => GpuBackend::Metal,
                        sefai_engine_llama::GpuBackend::Cuda => GpuBackend::Cuda,
                        sefai_engine_llama::GpuBackend::Vulkan => GpuBackend::Vulkan,
                    },
                },
            },
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(..) => Device::Mlx,
            #[cfg(feature = "onnx")]
            Self::Onnx(model, _) => Device::Onnx {
                provider: match model.provider() {
                    sefai_engine_onnx::Provider::Cpu => OnnxProvider::Cpu,
                    sefai_engine_onnx::Provider::WebGpu => OnnxProvider::WebGpu,
                },
                threads: model.threads(),
            },
        }
    }
    pub fn context_tokens_max(&self) -> u32 {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(model) => model.context_tokens_max(),
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(model, _) => model.context_tokens_max(),
            #[cfg(feature = "onnx")]
            Self::Onnx(model, _) => model.context_tokens_max(),
        }
    }
    pub fn prepare(&self, prompt: &Prompt) -> Result<Input<'_>> {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(model) => Ok(Input::Llama(model.prepare(prompt)?)),
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(model, _) => Ok(Input::Mlx(model.prepare(prompt)?)),
            #[cfg(feature = "onnx")]
            Self::Onnx(model, _) => Ok(Input::Onnx(model.prepare(prompt)?)),
        }
    }
    pub fn prepare_raw(&self, text: &str) -> Result<Input<'_>> {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(model) => Ok(Input::Llama(model.prepare_raw(text)?)),
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(model, _) => Ok(Input::Mlx(model.prepare_raw(text)?)),
            #[cfg(feature = "onnx")]
            Self::Onnx(model, _) => Ok(Input::Onnx(model.prepare_raw(text)?)),
        }
    }
    pub fn generate(
        &self,
        input: &Input<'_>,
        settings: Settings,
        cancelled: impl FnMut() -> bool,
        emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics> {
        self.generate_cached(&mut Cache::default(), input, settings, cancelled, emit)
    }
    pub fn generate_cached<'model>(
        &'model self,
        cache: &mut Cache<'model>,
        input: &Input<'_>,
        settings: Settings,
        cancelled: impl FnMut() -> bool,
        emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics> {
        match (self, input) {
            #[cfg(feature = "gguf")]
            (Self::Llama(model), Input::Llama(input)) => {
                Ok(model.generate_cached(&mut cache.llama, input, settings, cancelled, emit)?)
            }
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            (Self::Mlx(model, _), Input::Mlx(input)) => {
                Ok(model.generate_cached(&mut cache.mlx, input, settings, cancelled, emit)?)
            }
            #[cfg(feature = "onnx")]
            (Self::Onnx(model, _), Input::Onnx(input)) => {
                Ok(model.generate_cached(&mut cache.onnx, input, settings, cancelled, emit)?)
            }
            // Reachable only when the build compiles more than one engine.
            #[allow(unreachable_patterns)]
            _ => {
                let _ = (cache, settings, cancelled, emit);
                Err("input belongs to a different engine".into())
            }
        }
    }
    pub fn memory(&self) -> String {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(_) => "memory reporting unavailable".into(),
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(..) => match sefai_engine_mlx::memory() {
                Ok([active, cached, peak]) => format!(
                    "mlx_active_bytes={active} mlx_cached_bytes={cached} mlx_peak_active_bytes={peak}; process-global allocator counters, host RSS excluded"
                ),
                Err(error) => format!("mlx memory reporting unavailable: {error}"),
            },
            #[cfg(feature = "onnx")]
            Self::Onnx(..) => "onnx memory reporting unavailable".into(),
        }
    }

    pub fn reused_tokens(&self, cache: &Cache<'_>) -> usize {
        match self {
            #[cfg(feature = "gguf")]
            Self::Llama(_) => cache.llama.reused_tokens(),
            #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
            Self::Mlx(..) => cache.mlx.reused_tokens(),
            #[cfg(feature = "onnx")]
            Self::Onnx(..) => cache.onnx.reused_tokens(),
        }
    }
}

#[cfg(not(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
)))]
pub struct Model<'engine>(&'engine std::convert::Infallible);
#[cfg(not(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
)))]
pub struct Input<'model>(&'model std::convert::Infallible);
#[cfg(not(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
)))]
impl Input<'_> {
    pub fn token_count(&self) -> usize {
        match *self.0 {}
    }
}
#[cfg(not(any(
    feature = "gguf",
    feature = "onnx",
    all(feature = "mlx", target_os = "macos", target_arch = "aarch64")
)))]
impl Model<'_> {
    pub fn device(&self) -> Device {
        match *self.0 {}
    }
    pub fn context_tokens_max(&self) -> u32 {
        match *self.0 {}
    }
    pub fn prepare(&self, _prompt: &Prompt) -> Result<Input<'_>> {
        match *self.0 {}
    }
    pub fn prepare_raw(&self, _text: &str) -> Result<Input<'_>> {
        match *self.0 {}
    }
    pub fn generate(
        &self,
        _input: &Input<'_>,
        _settings: Settings,
        _cancelled: impl FnMut() -> bool,
        _emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics> {
        match *self.0 {}
    }
    pub fn generate_cached<'model>(
        &'model self,
        _cache: &mut Cache<'model>,
        _input: &Input<'_>,
        _settings: Settings,
        _cancelled: impl FnMut() -> bool,
        _emit: impl FnMut(&str) -> io::Result<()>,
    ) -> Result<Statistics> {
        match *self.0 {}
    }
    pub fn reused_tokens(&self, _cache: &Cache<'_>) -> usize {
        match *self.0 {}
    }
    pub fn memory(&self) -> String {
        match *self.0 {}
    }
}

#[cfg(all(test, any(feature = "gguf", feature = "mlx")))]
mod tests {
    use super::*;

    #[cfg(feature = "gguf")]
    #[test]
    fn non_gguf_file_is_rejected_before_native_loading() {
        let path = std::env::temp_dir().join(format!("sefai-not-gguf-{}", std::process::id()));
        std::fs::write(&path, b"ONNX").expect("fixture");
        let mut engine = Engine::new().expect("engine");
        let result = engine.load(
            &path,
            Some(Format::Gguf),
            Device::Cpu,
            Arc::new(AtomicBool::new(false)),
        );
        std::fs::remove_file(&path).expect("remove fixture");
        assert!(matches!(result, Err(LoadError::UnsupportedArtifact(_))));
    }

    #[cfg(feature = "mlx")]
    #[test]
    fn declared_quantization_must_match_the_checkpoint() {
        use sefai_engine_mlx::Weights;
        let awq = Weights::Awq {
            bits: 4,
            group_size: 128,
        };
        assert!(matches!(
            admit_mlx_weights(Some(Format::Gptq), awq),
            Err(LoadError::UnsupportedArtifact(_))
        ));
        assert!(admit_mlx_weights(Some(Format::Awq), awq).is_ok());
        assert!(admit_mlx_weights(None, awq).is_ok());
        assert!(admit_mlx_weights(Some(Format::Mlx), Weights::Mlx).is_ok());
    }
}
