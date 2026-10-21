//! Fixed-workload qualification; runtime weights stay local and are not downloaded.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use sefai_core::Settings;
    use sefai_engine_mlx::{Cache, Model, memory};
    use std::{
        path::PathBuf,
        sync::{Arc, atomic::AtomicBool},
    };
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("specify the pinned model bundle")?,
    );
    let prefix = match std::env::args().nth(2).as_deref() {
        None => String::new(),
        Some("long") => "This is a local inference memory and throughput measurement with one sequence and full attention.\n".repeat(32),
        Some(_) => return Err("the optional workload must be long".into()),
    };
    let prompt = format!("{prefix}Continue this sequence, separated by commas: 1, 2, 3, 4, 5,");
    let settings = Settings {
        context_tokens: 2048,
        batch_tokens: 128,
        microbatch_tokens: 128,
        output_tokens: 128,
        threads: 1,
    };
    let model = Model::load(&path, Arc::new(AtomicBool::new(false)))?;
    let input = model.prepare_raw(&prompt)?;
    let ids = input.token_ids().collect::<Vec<_>>();
    let mut samples = Vec::new();
    for _ in 0..3 {
        let mut text = String::new();
        let statistics = model.generate_cached(
            &mut Cache::default(),
            &input,
            settings,
            || false,
            |piece| {
                text.push_str(piece);
                Ok(())
            },
        )?;
        samples.push(serde_json::json!({"prompt_tokens":statistics.prompt_tokens,"generated_tokens":statistics.generated_tokens,"prefill_s":statistics.prefill.as_secs_f64(),"generation_s":statistics.generation.as_secs_f64(),"stop":format!("{:?}",statistics.stop),"text":text,"memory_bytes":memory()?}));
    }
    drop(model);
    println!(
        "{}",
        serde_json::json!({"prompt":prompt,"prompt_ids":ids,"output_tokens":settings.output_tokens,"prefill_chunk_tokens":settings.microbatch_tokens,"samples":samples,"after_unload_memory_bytes":memory()?})
    );
    Ok(())
}
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn main() {
    eprintln!("native mlx requires Apple Silicon macos");
    std::process::exit(1);
}
