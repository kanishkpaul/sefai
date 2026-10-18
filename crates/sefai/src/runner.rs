use crate::term::Output;
use clap::{Args, ValueEnum};
use sefai_core::{Message, Prompt};
use sefai_engine_llama::{Device, Engine, GpuBackend, Settings, Stop};
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
    model: PathBuf,
    /// Prompt text, or - to read bounded UTF-8 from stdin.
    prompt: String,
    /// Bypass chat templating and tokenize literal text.
    #[arg(long)]
    raw: bool,
    #[arg(long, value_enum, default_value_t = Backend::Cpu)]
    backend: Backend,
    /// ggml device index from `sefai devices`; required for a GPU backend.
    #[arg(long)]
    device: Option<usize>,
    #[arg(long, default_value_t = 4096)]
    context: u32,
    #[arg(long, default_value_t = 512)]
    batch: u32,
    #[arg(long, default_value_t = 128)]
    microbatch: u32,
    #[arg(long, default_value_t = 128)]
    output: u32,
    #[arg(long)]
    threads: Option<u32>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Backend {
    Cpu,
    Metal,
    Cuda,
    Vulkan,
}

pub enum Mode {
    Reply,
    Benchmark(u32),
}

pub fn devices() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new()?;
    let mut terminal = Output::default();
    let mut output = io::stdout().lock();
    for device in engine.devices() {
        terminal.write(
            &format!("{}\t{}\t{}\n", device.index, device.backend, device.name),
            &mut output,
        )?;
    }
    Ok(())
}

pub fn run(options: Options, mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let repetitions = match mode {
        Mode::Reply => 1,
        Mode::Benchmark(repetitions) => repetitions,
    };
    let device = match (options.backend, options.device) {
        (Backend::Cpu, None) => Device::Cpu,
        (Backend::Cpu, Some(_)) => return Err("cpu mode does not accept --device".into()),
        (backend, Some(index)) => Device::Gpu {
            index,
            backend: match backend {
                Backend::Metal => GpuBackend::Metal,
                Backend::Cuda => GpuBackend::Cuda,
                Backend::Vulkan => GpuBackend::Vulkan,
                Backend::Cpu => unreachable!(),
            },
        },
        (_, None) => return Err("gpu mode requires --device from `sefai devices`".into()),
    };
    let mut prompt = options.prompt;
    if prompt == "-" {
        prompt.clear();
        io::stdin()
            .take(sefai_engine_llama::PROMPT_BYTES_MAX as u64 + 1)
            .read_to_string(&mut prompt)?;
    }
    if prompt.trim().is_empty() || prompt.len() > sefai_engine_llama::PROMPT_BYTES_MAX {
        return Err("prompt must contain text within 1048576 bytes".into());
    }
    let settings = Settings {
        context_tokens: options.context,
        batch_tokens: options.batch,
        microbatch_tokens: options.microbatch,
        output_tokens: options.output,
        threads: options.threads.unwrap_or(Settings::default().threads),
    };
    settings.validate(1, u32::MAX)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    let mut engine = Engine::new()?;
    let loading = Instant::now();
    let model = engine.load(&options.model, device, cancelled.clone())?;
    let load_seconds = loading.elapsed().as_secs_f64();
    let input = if options.raw {
        model.prepare_raw(&prompt)?
    } else {
        model.prepare(&Prompt::new(vec![Message::user(prompt)]))?
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
        } else {
            writeln!(output)?;
        }
        if statistics.stop == Stop::Cancelled {
            return Err("generation cancelled".into());
        }
    }
    Ok(())
}
