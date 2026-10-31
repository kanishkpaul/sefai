use crate::engine::{Device, Engine, GpuBackend, OnnxProvider, Settings, Stop};
use crate::term::Output;
use crate::workspace::{Backend, Format, Provider, Runner, Selection};
use clap::Args;
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[derive(Args)]
pub struct Options {
    #[arg(long)]
    model: Option<PathBuf>,
    /// Prompt text, or - to read bounded UTF-8 from stdin.
    prompt: String,
    /// Bypass chat templating and tokenize literal text.
    #[arg(long)]
    raw: bool,
    /// Declared model format, checked against the artifact before loading.
    #[arg(long, value_enum)]
    format: Option<Format>,
    #[arg(long, value_enum)]
    backend: Option<Backend>,
    /// ONNX Runtime execution provider; requires --backend onnx.
    #[arg(long, value_enum)]
    provider: Option<Provider>,
    /// ggml device index from `sefai devices`; required for a GPU backend.
    #[arg(long)]
    device: Option<usize>,
    #[arg(long)]
    context: Option<u32>,
    #[arg(long)]
    batch: Option<u32>,
    #[arg(long)]
    microbatch: Option<u32>,
    #[arg(long)]
    output: Option<u32>,
    #[arg(long)]
    threads: Option<u32>,
}

/// One model load: the declared format, the engine device and the bounded settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub format: Option<Format>,
    pub device: Device,
    pub settings: Settings,
}

pub enum Mode {
    Reply,
    Benchmark(u32),
}

pub fn devices() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "gguf")]
    {
        let engine = sefai_engine_llama::Engine::new()?;
        let mut terminal = Output::default();
        let mut output = io::stdout().lock();
        for device in engine.devices() {
            terminal.write(
                &format!("{}\t{}\t{}\n", device.index, device.backend, device.name),
                &mut output,
            )?;
        }
    }
    #[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
    println!("mlx\tMetal\tcompiled MLX target; availability checked on load");
    #[cfg(feature = "onnx")]
    println!("onnx\tCPU\tcompiled ONNX Runtime CPU execution provider");
    #[cfg(feature = "onnx-webgpu")]
    println!(
        "onnx\tWebGPU\tcompiled ONNX Runtime WebGPU execution provider; availability checked on load"
    );
    Ok(())
}

pub fn run(
    options: Options,
    mode: Mode,
    selection: &Selection,
) -> Result<(), Box<dyn std::error::Error>> {
    let repetitions = match mode {
        Mode::Reply => 1,
        Mode::Benchmark(repetitions) => repetitions,
    };
    let overrides = Runner {
        backend: options.backend,
        provider: options.provider,
        device: options.device,
        context: options.context,
        batch: options.batch,
        microbatch: options.microbatch,
        output: options.output,
        threads: options.threads,
    };
    let mut prompt = options.prompt;
    if prompt == "-" {
        prompt.clear();
        io::stdin()
            .take(crate::engine::PROMPT_BYTES_MAX as u64 + 1)
            .read_to_string(&mut prompt)?;
    }
    if prompt.trim().is_empty() || prompt.len() > crate::engine::PROMPT_BYTES_MAX {
        return Err("prompt must contain text within 1048576 bytes".into());
    }
    let prepared = selection.prepare(prompt)?;
    // A profile's declared format describes its own path, not one given with --model.
    let declared = prepared.format.filter(|_| options.model.is_none());
    let plan = resolve(overrides, prepared.runner, options.format.or(declared))?;
    if options.raw
        && (!prepared.prompt.sections().is_empty() || prepared.prompt.conversation().len() != 1)
    {
        return Err("raw mode cannot discard workspace instructions or session history".into());
    }
    let path = options
        .model
        .or(prepared.model)
        .ok_or("model: specify --model or a Markdown profile")?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    let mut engine = Engine::new()?;
    let loading = Instant::now();
    let model = engine.load(&path, plan.format, plan.device, cancelled.clone())?;
    let load_seconds = loading.elapsed().as_secs_f64();
    let input = if options.raw {
        let message = prepared
            .prompt
            .conversation()
            .last()
            .ok_or("prompt: missing user message")?;
        model.prepare_raw(&message.content)?
    } else {
        model.prepare(&prepared.prompt)?
    };
    let mut output = io::stdout().lock();
    let mut terminal = Output::default();
    eprintln!(
        "local · {} · context {} · prompt {} tokens",
        model.device(),
        plan.settings.context_tokens,
        input.token_count()
    );
    for repetition in 1..=repetitions {
        let statistics = model.generate(
            &input,
            plan.settings,
            || cancelled.load(Ordering::Relaxed),
            |text| {
                if matches!(mode, Mode::Benchmark(_)) {
                    Ok(())
                } else {
                    terminal.write(text, &mut output)
                }
            },
        )?;
        if matches!(mode, Mode::Benchmark(_)) {
            writeln!(
                output,
                "sample={repetition} load_s={load_seconds:.6} prompt_tokens={} generated_tokens={} prefill_s={:.6} generation_s={:.6} stop={:?}",
                statistics.prompt_tokens,
                statistics.generated_tokens,
                statistics.prefill.as_secs_f64(),
                statistics.generation.as_secs_f64(),
                statistics.stop
            )?;
            eprintln!("{}", model.memory());
        } else {
            writeln!(output)?;
        }
        if statistics.stop == Stop::Cancelled {
            return Err("generation cancelled".into());
        }
    }
    Ok(())
}

/// Resolves runner settings and checks that the declared format can run on the selected backend.
/// A declared format without a backend selects its engine; GGUF still defaults to the CPU.
pub(crate) fn resolve(
    overrides: Runner,
    mut configured: Runner,
    format: Option<Format>,
) -> Result<Plan, Box<dyn std::error::Error>> {
    configured.overlay(&overrides);
    let implied = match format {
        Some(Format::Mlx | Format::Gptq | Format::Awq) => Some(Backend::Mlx),
        Some(Format::Onnx) => Some(Backend::Onnx),
        Some(Format::Gguf) | None => None,
    };
    configured.backend = configured.backend.or(implied);
    if let Some(format) = format {
        let backend = configured.backend.unwrap_or(Backend::Cpu);
        let compatible = match format {
            Format::Gguf => !matches!(backend, Backend::Mlx | Backend::Onnx),
            Format::Mlx | Format::Gptq | Format::Awq => backend == Backend::Mlx,
            Format::Onnx => backend == Backend::Onnx,
        };
        if !compatible {
            return Err(format!(
                "format {} cannot run on backend {}; no automatic conversion",
                format.name(),
                backend.name()
            )
            .into());
        }
    }
    if configured.provider.is_some() && configured.backend != Some(Backend::Onnx) {
        return Err("provider requires backend onnx".into());
    }
    let defaults = Settings::default();
    let mut settings = Settings {
        context_tokens: configured.context.unwrap_or(defaults.context_tokens),
        batch_tokens: configured.batch.unwrap_or(defaults.batch_tokens),
        microbatch_tokens: configured.microbatch.unwrap_or(defaults.microbatch_tokens),
        output_tokens: configured.output.unwrap_or(defaults.output_tokens),
        threads: configured.threads.unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, |count| count.get().min(256) as u32)
        }),
    };
    let device = match (
        configured.backend.unwrap_or(Backend::Cpu),
        configured.device,
    ) {
        (Backend::Mlx, None) => Device::Mlx,
        (Backend::Mlx, Some(_)) => {
            return Err("mlx selects the Apple GPU and does not accept --device".into());
        }
        (Backend::Onnx, None) => Device::Onnx {
            provider: match configured.provider.unwrap_or(Provider::Cpu) {
                Provider::Cpu => OnnxProvider::Cpu,
                Provider::Webgpu => OnnxProvider::WebGpu,
            },
            threads: settings.threads,
        },
        (Backend::Onnx, Some(_)) => {
            return Err("onnx selects an execution provider and does not accept --device".into());
        }
        (Backend::Cpu, None) => Device::Cpu,
        (Backend::Cpu, Some(_)) => return Err("cpu mode does not accept --device".into()),
        (backend, Some(index)) => Device::Gpu {
            index,
            backend: match backend {
                Backend::Metal => GpuBackend::Metal,
                Backend::Cuda => GpuBackend::Cuda,
                Backend::Vulkan => GpuBackend::Vulkan,
                Backend::Cpu | Backend::Mlx | Backend::Onnx => unreachable!(),
            },
        },
        (_, None) => return Err("gpu mode requires --device from `sefai devices`".into()),
    };
    // MLX and ONNX prefill in microbatch chunks with no separate batch, and MLX schedules its
    // own threads; an explicit value for a control the engine never applies is an error.
    let (engine, unused) = match device {
        Device::Mlx => (
            "mlx",
            [("batch", configured.batch), ("threads", configured.threads)],
        ),
        Device::Onnx { .. } => ("onnx", [("batch", configured.batch), ("threads", None)]),
        Device::Cpu | Device::Gpu { .. } => ("gguf", [("batch", None), ("threads", None)]),
    };
    if let Some((field, _)) = unused.iter().find(|(_, value)| value.is_some()) {
        return Err(format!(
            "runner {field} is not used by the {engine} engine; remove it from the profile, config.md or command"
        )
        .into());
    }
    if engine != "gguf" {
        settings.batch_tokens = settings.microbatch_tokens;
    }
    settings.validate(1, u32::MAX)?;
    Ok(Plan {
        format,
        device,
        settings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_cpu_clears_inherited_gpu_selection() {
        let configured = Runner {
            backend: Some(Backend::Cuda),
            device: Some(2),
            ..Runner::default()
        };
        let overrides = Runner {
            backend: Some(Backend::Cpu),
            ..Runner::default()
        };
        assert_eq!(
            resolve(overrides, configured, None)
                .expect("resolve")
                .device,
            Device::Cpu
        );
    }

    #[test]
    fn explicit_runner_flags_override_markdown_values() {
        let configured = Runner {
            threads: Some(3),
            output: Some(16),
            ..Runner::default()
        };
        let overrides = Runner {
            threads: Some(4),
            output: Some(0),
            ..Runner::default()
        };
        let settings = resolve(overrides, configured, None)
            .expect("resolve")
            .settings;
        assert_eq!((settings.threads, settings.output_tokens), (4, 0));
    }

    #[test]
    fn explicit_cpu_with_explicit_gpu_index_is_rejected() {
        let overrides = Runner {
            backend: Some(Backend::Cpu),
            device: Some(2),
            ..Runner::default()
        };
        assert!(resolve(overrides, Runner::default(), None).is_err());
    }

    #[test]
    fn onnx_binds_the_resolved_thread_count_at_load() {
        let overrides = Runner {
            backend: Some(Backend::Onnx),
            threads: Some(3),
            ..Runner::default()
        };
        assert_eq!(
            resolve(overrides, Runner::default(), None)
                .expect("resolve")
                .device,
            Device::Onnx {
                provider: OnnxProvider::Cpu,
                threads: 3
            }
        );
    }

    #[test]
    fn provider_without_onnx_backend_is_rejected() {
        let overrides = Runner {
            provider: Some(Provider::Webgpu),
            ..Runner::default()
        };
        assert!(resolve(overrides, Runner::default(), None).is_err());
    }

    #[test]
    fn onnx_selects_the_requested_provider() {
        let overrides = Runner {
            backend: Some(Backend::Onnx),
            provider: Some(Provider::Webgpu),
            threads: Some(2),
            ..Runner::default()
        };
        assert_eq!(
            resolve(overrides, Runner::default(), None)
                .expect("resolve")
                .device,
            Device::Onnx {
                provider: OnnxProvider::WebGpu,
                threads: 2
            }
        );
    }

    #[test]
    fn declared_format_selects_its_engine() {
        for (format, device) in [
            (Format::Gptq, Device::Mlx),
            (Format::Awq, Device::Mlx),
            (Format::Mlx, Device::Mlx),
            (Format::Gguf, Device::Cpu),
        ] {
            assert_eq!(
                resolve(Runner::default(), Runner::default(), Some(format))
                    .expect("resolve")
                    .device,
                device
            );
        }
    }

    #[test]
    fn declared_format_rejects_another_engine() {
        for (format, backend) in [
            (Format::Gptq, Backend::Cpu),
            (Format::Awq, Backend::Onnx),
            (Format::Onnx, Backend::Mlx),
            (Format::Gguf, Backend::Mlx),
        ] {
            let overrides = Runner {
                backend: Some(backend),
                ..Runner::default()
            };
            assert!(resolve(overrides, Runner::default(), Some(format)).is_err());
        }
    }

    #[test]
    fn controls_an_engine_never_applies_are_rejected() {
        for (backend, field) in [
            (Backend::Mlx, "batch"),
            (Backend::Mlx, "threads"),
            (Backend::Onnx, "batch"),
        ] {
            let overrides = Runner {
                backend: Some(backend),
                batch: (field == "batch").then_some(64),
                threads: (field == "threads").then_some(2),
                ..Runner::default()
            };
            let error = resolve(overrides, Runner::default(), None).expect_err(field);
            assert!(error.to_string().contains(field), "{error}");
        }
    }

    #[test]
    fn microbatch_engines_are_not_bounded_by_the_default_batch() {
        let overrides = Runner {
            backend: Some(Backend::Mlx),
            microbatch: Some(1024),
            ..Runner::default()
        };
        let settings = resolve(overrides, Runner::default(), None)
            .expect("resolve")
            .settings;
        assert_eq!(settings.batch_tokens, 1024);
    }
}
