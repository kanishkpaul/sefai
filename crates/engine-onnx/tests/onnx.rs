use sefai_core::{Message, Prompt, Settings, Stop};
use sefai_engine_onnx::{Cache, Input, Model};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

fn path() -> PathBuf {
    PathBuf::from(
        std::env::var_os("SEFAI_ONNX_TEST_MODEL")
            .expect("set SEFAI_ONNX_TEST_MODEL to the pinned graph"),
    )
}
fn model() -> Model {
    Model::load(&path(), 2, Arc::new(AtomicBool::new(false))).expect("load real ONNX graph")
}
fn settings(model: &Model, output_tokens: u32) -> Settings {
    Settings {
        context_tokens: 2048,
        batch_tokens: 128,
        microbatch_tokens: 64,
        output_tokens,
        threads: model.threads(),
    }
}
#[test]
fn real_graph_streams_a_bounded_chat_reply() {
    let model = model();
    let input = model
        .prepare(&Prompt::new(vec![Message::user(
            "Reply with one short greeting.",
        )]))
        .expect("template");
    let mut text = String::new();
    let stats = model
        .generate_cached(
            &mut Cache::default(),
            &input,
            settings(&model, 24),
            || false,
            |s| {
                text.push_str(s);
                Ok(())
            },
        )
        .expect("generate");
    assert!(!text.trim().is_empty());
    assert!(stats.generated_tokens <= 24);
    assert_ne!(stats.stop, Stop::Cancelled);
}

#[test]
fn unicode_survives_tokenization_and_streaming() {
    let model = model();
    let input = model
        .prepare_raw("こんにちは、こんにちは、こんにちは、こんにちは、")
        .expect("unicode tokens");
    let mut text = String::new();
    model
        .generate_cached(
            &mut Cache::default(),
            &input,
            settings(&model, 16),
            || false,
            |s| {
                text.push_str(s);
                Ok(())
            },
        )
        .expect("generate");
    assert!(text.contains("こんにちは"), "{text}");
}

#[test]
fn an_extended_prompt_reuses_cache_and_matches_a_cold_rebuild() {
    let model = model();
    let first = model
        .prepare_raw("The capital of France is")
        .expect("first input");
    let second = model
        .prepare_raw("The capital of France is Paris. The capital of Italy is")
        .expect("extended input");
    fn generate<'model>(
        model: &'model Model,
        cache: &mut Cache<'model>,
        input: &Input<'_>,
        output_tokens: u32,
    ) -> String {
        let mut text = String::new();
        model
            .generate_cached(
                cache,
                input,
                settings(model, output_tokens),
                || false,
                |s| {
                    text.push_str(s);
                    Ok(())
                },
            )
            .expect("generate");
        text
    }
    let cold = generate(&model, &mut Cache::default(), &second, 8);
    let mut cache = Cache::default();
    // One output token leaves exactly the first prompt cached.
    generate(&model, &mut cache, &first, 1);
    let warm = generate(&model, &mut cache, &second, 8);
    assert!(cache.reused_tokens() > 0);
    assert_eq!(warm, cold);
}

#[test]
fn changed_prompt_discards_the_cache() {
    let model = model();
    let first = model
        .prepare_raw("The capital of France is")
        .expect("first input");
    let second = model.prepare_raw("Name a fruit:").expect("changed input");
    let mut cache = Cache::default();
    for input in [&first, &second] {
        model
            .generate_cached(&mut cache, input, settings(&model, 1), || false, |_| Ok(()))
            .expect("generation");
    }
    assert_eq!(cache.reused_tokens(), 0);
}

#[test]
fn cancellation_invalidates_cached_state() {
    let model = model();
    let input = model.prepare_raw("Count from one to ten:").expect("tokens");
    let mut cache = Cache::default();
    let mut calls = 0;
    let stats = model
        .generate_cached(
            &mut cache,
            &input,
            settings(&model, 32),
            || {
                calls += 1;
                calls > 3
            },
            |_| Ok(()),
        )
        .expect("cancelled generation");
    assert_eq!(stats.stop, Stop::Cancelled);
    assert_eq!(cache.reused_tokens(), 0);
}

#[test]
fn callback_failure_invalidates_cached_state() {
    let model = model();
    let input = model.prepare_raw("Count from one to ten:").expect("tokens");
    let mut cache = Cache::default();
    assert!(
        model
            .generate_cached(
                &mut cache,
                &input,
                settings(&model, 8),
                || false,
                |_| { Err(std::io::Error::other("closed output")) }
            )
            .is_err()
    );
    let again = model
        .generate_cached(
            &mut cache,
            &input,
            settings(&model, 1),
            || false,
            |_| Ok(()),
        )
        .expect("rebuild");
    assert_eq!(
        (cache.reused_tokens(), again.prompt_tokens),
        (0, input.token_count())
    );
}

#[test]
fn context_reservation_fails_before_streaming() {
    let model = model();
    let input = model.prepare_raw("hello").expect("tokens");
    let mut settings = settings(&model, 16);
    settings.context_tokens = input.token_count() as u32 + 8;
    let mut emitted = false;
    assert!(
        model
            .generate_cached(
                &mut Cache::default(),
                &input,
                settings,
                || false,
                |_| {
                    emitted = true;
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!emitted);
}

#[test]
fn changed_threads_are_rejected() {
    let model = model();
    let input = model.prepare_raw("hello").expect("tokens");
    let mut settings = settings(&model, 1);
    settings.threads = model.threads() % 256 + 1;
    assert!(
        model
            .generate_cached(
                &mut Cache::default(),
                &input,
                settings,
                || false,
                |_| Ok(())
            )
            .is_err()
    );
}

#[test]
fn incompatible_graph_signature_is_rejected() {
    let source = path().canonicalize().expect("graph");
    let root = std::env::temp_dir().join(format!("sefai-onnx-signature-{}", std::process::id()));
    fs::create_dir_all(&root).expect("fixture");
    let metadata = if source.parent().expect("dir").join("config.json").exists() {
        source.parent().expect("dir").to_path_buf()
    } else {
        source
            .parent()
            .and_then(|dir| dir.parent())
            .expect("bundle")
            .to_path_buf()
    };
    for name in [
        "generation_config.json",
        "tokenizer.json",
        "tokenizer_config.json",
    ] {
        fs::copy(metadata.join(name), root.join(name)).expect("metadata");
    }
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(metadata.join("config.json")).expect("config"))
            .expect("json");
    config["num_hidden_layers"] =
        (config["num_hidden_layers"].as_u64().expect("layers") + 1).into();
    fs::write(root.join("config.json"), config.to_string()).expect("config");
    let graph = root.join("model.onnx");
    fs::hard_link(&source, &graph)
        .or_else(|_| fs::copy(&source, &graph).map(|_| ()))
        .expect("graph");
    let result = Model::load(&graph, 2, Arc::new(AtomicBool::new(false)));
    fs::remove_dir_all(&root).expect("remove fixture");
    let error = result.err().expect("signature mismatch").to_string();
    assert!(error.contains("past_key_values"), "{error}");
}

#[test]
fn missing_external_data_fails_at_load() {
    let source = PathBuf::from(
        std::env::var_os("SEFAI_ONNX_TEST_EXTERNAL_MODEL")
            .expect("set SEFAI_ONNX_TEST_EXTERNAL_MODEL to the pinned external-data graph"),
    )
    .canonicalize()
    .expect("graph");
    let directory = source.parent().expect("graph directory");
    assert!(
        directory
            .join(format!(
                "{}_data",
                source.file_name().expect("name").to_string_lossy()
            ))
            .exists(),
        "fixture must carry external data"
    );
    let metadata = if directory.join("config.json").exists() {
        directory
    } else {
        directory.parent().expect("bundle")
    };
    let root = std::env::temp_dir().join(format!("sefai-onnx-external-{}", std::process::id()));
    fs::create_dir_all(&root).expect("fixture");
    for name in [
        "config.json",
        "generation_config.json",
        "tokenizer.json",
        "tokenizer_config.json",
    ] {
        fs::copy(metadata.join(name), root.join(name)).expect("metadata");
    }
    let graph = root.join(source.file_name().expect("name"));
    fs::hard_link(&source, &graph)
        .or_else(|_| fs::copy(&source, &graph).map(|_| ()))
        .expect("graph");
    let result = Model::load(&graph, 2, Arc::new(AtomicBool::new(false)));
    fs::remove_dir_all(&root).expect("remove fixture");
    assert!(result.is_err());
}
