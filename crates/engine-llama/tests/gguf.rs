use sefai_core::{Message, Prompt};
use sefai_engine_llama::{Device, Engine, GpuBackend, Settings, Stop};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

static BACKEND: Mutex<()> = Mutex::new(());

fn fixture() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("SEFAI_TEST_MODEL")
            .expect("SEFAI_TEST_MODEL is required for the gguf integration target"),
    );
    assert!(
        path.is_file(),
        "model fixture must exist: {}",
        path.display()
    );
    path
}

fn device() -> Device {
    match std::env::var("SEFAI_TEST_BACKEND").as_deref() {
        Ok("metal") => Device::Gpu {
            backend: GpuBackend::Metal,
            index: std::env::var("SEFAI_TEST_DEVICE")
                .expect("SEFAI_TEST_DEVICE is required for metal tests")
                .parse()
                .expect("device index"),
        },
        Ok("cpu") | Err(_) => Device::Cpu,
        other => panic!("unsupported test backend: {other:?}"),
    }
}

fn with_model(test: impl FnOnce(&sefai_engine_llama::Model<'_>)) {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    let model = engine
        .load(&fixture(), device(), Arc::new(AtomicBool::new(false)))
        .expect("load fixture");
    test(&model);
}

#[test]
fn prefill_crosses_batch_boundaries() {
    with_model(|model| {
        for count in [511, 512, 513, 3000] {
            let input = model.prepare_raw(&" a".repeat(count)).expect("raw prompt");
            assert_eq!(input.token_count(), count, "fixture tokenizer contract");
            let result = model
                .generate(
                    &input,
                    Settings {
                        output_tokens: 1,
                        ..Settings::default()
                    },
                    || false,
                    |_| Ok(()),
                )
                .expect("prefill");
            assert_eq!(result.prompt_tokens, count);
            assert_eq!(result.generated_tokens, 1);
        }
    });
}

#[test]
fn exact_context_boundary_generates() {
    with_model(|model| {
        let input = model.prepare_raw(&" a".repeat(511)).expect("prompt");
        let result = model
            .generate(
                &input,
                Settings {
                    context_tokens: 512,
                    output_tokens: 1,
                    ..Settings::default()
                },
                || false,
                |_| Ok(()),
            )
            .expect("exact boundary");
        assert_eq!(result.generated_tokens, 1);
    });
}

#[test]
fn context_overflow_fails_before_output() {
    with_model(|model| {
        let input = model.prepare_raw(&" a".repeat(512)).expect("prompt");
        let mut emitted = false;
        let result = model.generate(
            &input,
            Settings {
                context_tokens: 512,
                output_tokens: 1,
                ..Settings::default()
            },
            || false,
            |_| {
                emitted = true;
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(!emitted);
    });
}

#[test]
fn zero_output_never_decodes_or_emits() {
    with_model(|model| {
        let input = model.prepare_raw("hello").expect("prompt");
        let result = model
            .generate(
                &input,
                Settings {
                    output_tokens: 0,
                    ..Settings::default()
                },
                || false,
                |_| panic!("zero output emitted text"),
            )
            .expect("zero output");
        assert_eq!(result.generated_tokens, 0);
        assert_eq!(result.prefill, std::time::Duration::ZERO);
    });
}

#[test]
fn cancellation_between_prefill_chunks_stops() {
    with_model(|model| {
        let input = model.prepare_raw(&" a".repeat(3000)).expect("prompt");
        let mut checks = 0;
        let result = model
            .generate(
                &input,
                Settings::default(),
                || {
                    checks += 1;
                    checks >= 3
                },
                |_| panic!("cancelled prefill emitted text"),
            )
            .expect("cancelled prefill");
        assert_eq!(result.stop, Stop::Cancelled);
        assert_eq!(result.generated_tokens, 0);
    });
}

#[test]
fn cancellation_after_output_stops_next_decode() {
    with_model(|model| {
        let input = model
            .prepare(&Prompt::new(vec![Message::user(
                "List the numbers from one to ten.",
            )]))
            .expect("prompt");
        let cancelled = AtomicBool::new(false);
        let result = model
            .generate(
                &input,
                Settings::default(),
                || cancelled.load(Ordering::Relaxed),
                |_| {
                    cancelled.store(true, Ordering::Relaxed);
                    Ok(())
                },
            )
            .expect("cancelled output");
        assert_eq!(result.stop, Stop::Cancelled);
        assert_eq!(result.generated_tokens, 1);
    });
}

#[test]
fn template_preserves_unicode_and_one_system_message() {
    with_model(|model| {
        let mut prompt = Prompt::new(vec![
            Message::system("Keep it brief."),
            Message::user("বাংলা 🙂 e\u{301}"),
        ]);
        prompt.add_section("system", "You are helpful.");
        let input = model.prepare(&prompt).expect("native template");
        let mut output = String::new();
        model
            .generate(
                &input,
                Settings {
                    output_tokens: 16,
                    ..Settings::default()
                },
                || false,
                |part| {
                    output.push_str(part);
                    Ok(())
                },
            )
            .expect("unicode generation");
        assert!(!output.contains('\u{fffd}'));
        assert!(!output.contains("<|im_end|>"));
    });
}

#[test]
fn invalid_device_is_rejected_without_fallback() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    assert!(
        engine
            .load(
                &fixture(),
                Device::Gpu {
                    backend: GpuBackend::Metal,
                    index: usize::MAX
                },
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
}

#[test]
fn repeated_model_load_and_unload_succeeds() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    for _ in 0..3 {
        let model = engine
            .load(&fixture(), device(), Arc::new(AtomicBool::new(false)))
            .expect("reload");
        let input = model.prepare_raw("hello").expect("prompt");
        model
            .generate(
                &input,
                Settings {
                    output_tokens: 1,
                    ..Settings::default()
                },
                || false,
                |_| Ok(()),
            )
            .expect("generation after reload");
    }
}

#[test]
fn output_failure_is_reported() {
    with_model(|model| {
        let input = model.prepare_raw("hello").expect("prompt");
        assert!(
            model
                .generate(
                    &input,
                    Settings::default(),
                    || false,
                    |_| Err(std::io::ErrorKind::BrokenPipe.into())
                )
                .is_err()
        );
    });
}

#[test]
fn empty_prompt_is_rejected() {
    with_model(|model| assert!(model.prepare_raw(" \n").is_err()));
}
