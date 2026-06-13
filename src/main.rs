use anyhow::{bail, Context, Result};
use clap::Parser;
use llama_cpp_4::context::params::LlamaContextParams;
use llama_cpp_4::llama_backend::LlamaBackend;
use llama_cpp_4::llama_batch::LlamaBatch;
use llama_cpp_4::model::params::LlamaModelParams;
use llama_cpp_4::model::{AddBos, LlamaModel, Special};
use llama_cpp_4::sampling::LlamaSampler;
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "A tiny GGUF text generation runner")]
struct Args {
    /// Path to a local GGUF model file.
    model: PathBuf,

    /// Prompt to feed into the model.
    #[arg(short, long)]
    prompt: String,

    /// Maximum number of tokens to generate.
    #[arg(short = 'n', long, default_value_t = 128)]
    max_tokens: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if !args.model.exists() {
        bail!("model file does not exist: {}", args.model.display());
    }

    let backend = LlamaBackend::init().context("failed to initialize llama backend")?;

    let model = LlamaModel::load_from_file(&backend, &args.model, &LlamaModelParams::default())
        .with_context(|| format!("failed to load model from {}", args.model.display()))?;

    let ctx_params =
        LlamaContextParams::default().with_n_ctx(Some(NonZeroU32::new(2048).unwrap()));
    let mut ctx = model
        .new_context(&backend, ctx_params)
        .context("failed to create llama context")?;

    let tokens = model
        .str_to_token(&args.prompt, AddBos::Always)
        .context("failed to tokenize prompt")?;

    let mut batch = LlamaBatch::new(512, 1);
    let last_index = (tokens.len() - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens.iter().copied()) {
        batch
            .add(token, i, &[0], i == last_index)
            .context("failed to add prompt token to batch")?;
    }
    ctx.decode(&mut batch)
        .context("failed to decode prompt batch")?;

    print!("{}", args.prompt);
    io::stdout().flush().context("failed to flush stdout")?;

    let mut generated = 0usize;
    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);

    while generated < args.max_tokens {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if model.is_eog_token(token) {
            break;
        }

        let bytes = model
            .token_to_bytes(token, Special::Tokenize)
            .context("failed to convert token to bytes")?;
        let piece = String::from_utf8_lossy(&bytes);
        print!("{piece}");
        io::stdout().flush().context("failed to flush stdout")?;

        batch.clear();
        batch
            .add(token, (last_index + 1) + generated as i32, &[0], true)
            .context("failed to add generated token to batch")?;
        ctx.decode(&mut batch)
            .context("failed to decode generated token")?;
        generated += 1;
    }

    println!();
    Ok(())
}
