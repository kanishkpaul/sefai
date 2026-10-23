use crate::engine::{Device, Engine, GpuBackend, Settings, Stop};
use crate::term::Output;
use crate::workspace::{Backend, Runner, Selection};
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
    #[arg(long, value_enum)]
    backend: Option<Backend>,
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
    let (device, settings) = resolve(overrides, prepared.runner)?;
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
    let model = engine.load(&path, device, cancelled.clone())?;
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
        "local · {:?} · context {} · prompt {} tokens",
        model.device(),
        settings.context_tokens,
        input.token_count()
    );
    for repetition in 1..=repetitions {
        let statistics = model.generate(
            &input,
            settings,
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

pub(crate) fn resolve(
    overrides: Runner,
    mut configured: Runner,
) -> Result<(Device, Settings), Box<dyn std::error::Error>> {
    configured.overlay(&overrides);
    let defaults = Settings::default();
    let settings = Settings {
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
            threads: settings.threads,
        },
        (Backend::Onnx, Some(_)) => {
            return Err(
                "onnx selects the cpu execution provider and does not accept --device".into(),
            );
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
    settings.validate(1, u32::MAX)?;
    Ok((device, settings))
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
            resolve(overrides, configured).expect("resolve").0,
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
        let (_, settings) = resolve(overrides, configured).expect("resolve");
        assert_eq!((settings.threads, settings.output_tokens), (4, 0));
    }

    #[test]
    fn explicit_cpu_with_explicit_gpu_index_is_rejected() {
        let overrides = Runner {
            backend: Some(Backend::Cpu),
            device: Some(2),
            ..Runner::default()
        };
        assert!(resolve(overrides, Runner::default()).is_err());
    }

    #[test]
    fn onnx_binds_the_resolved_thread_count_at_load() {
        let overrides = Runner {
            backend: Some(Backend::Onnx),
            threads: Some(3),
            ..Runner::default()
        };
        assert_eq!(
            resolve(overrides, Runner::default()).expect("resolve").0,
            Device::Onnx { threads: 3 }
        );
    }
}
