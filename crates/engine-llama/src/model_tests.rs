use super::*;
use llama_cpp_2::{
    model::params::kv_overrides::ParamOverrideValue,
    token::{data::LlamaTokenData, data_array::LlamaTokenDataArray},
};
use std::{collections::BTreeSet, path::PathBuf, sync::Mutex};

static BACKEND: Mutex<()> = Mutex::new(());

fn fixture() -> PathBuf {
    std::env::var_os("SEFAI_TEST_MODEL")
        .expect("SEFAI_TEST_MODEL is required for model-tests")
        .into()
}

#[test]
fn gguf_sample_accepts_a_token_once() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    let model = engine
        .load(&fixture(), test_device(), Arc::new(AtomicBool::new(false)))
        .expect("model");
    let input = model.prepare_raw("hello").expect("prompt");
    let mut context = model
        .model
        .new_context(
            &model.engine.backend,
            context_params(Settings::default(), model.device),
        )
        .expect("context");
    let mut batch = LlamaBatch::new(512, 1);
    batch.add_sequence(&input.tokens, 0, false).expect("batch");
    context.decode(&mut batch).expect("decode");
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(model.model.vocab().n_tokens(), 64, 1.0, 1.0, 0.0),
        LlamaSampler::greedy(),
    ]);
    let token = sample(&mut sampler, &context);
    let mut candidates =
        LlamaTokenDataArray::new(vec![LlamaTokenData::new(token, 0.0, 0.0)], false);
    sampler.apply(&mut candidates);
    assert_eq!(
        candidates.data[0].logit(),
        -1.0,
        "frequency penalty counts exactly one acceptance"
    );
}

#[test]
fn gguf_recognizes_every_fixture_stop_token() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    let model = engine
        .load(&fixture(), test_device(), Arc::new(AtomicBool::new(false)))
        .expect("model");
    let vocab = model.model.vocab();
    let actual: BTreeSet<_> = vocab
        .tokens()
        .filter(|token| vocab.is_eog(*token))
        .map(|token| {
            vocab
                .text(token)
                .expect("stop token spelling")
                .to_str()
                .expect("utf8")
                .to_owned()
        })
        .collect();
    assert_eq!(
        actual,
        BTreeSet::from([
            "<|endoftext|>".into(),
            "<|im_end|>".into(),
            "<reponame>".into()
        ])
    );
}

#[test]
fn gguf_template_bos_is_not_duplicated() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::new().expect("backend");
    let mut params = Box::pin(model_params(Device::Cpu).expect("cpu params"));
    params.as_mut().append_kv_override(
        c"tokenizer.ggml.add_bos_token",
        ParamOverrideValue::Bool(true),
    );
    let native = LlamaModel::load_from_file(&engine.backend, fixture(), &params)
        .expect("overridden tokenizer");
    assert!(native.vocab().should_add_bos());
    let input = validate_text_and_tokenize(
        &native,
        "<|im_start|>user\nhello<|im_end|>\n<|im_start|>assistant\n",
        true,
    )
    .expect("chat input");
    assert_eq!(input.tokens[0], native.vocab().bos());
    assert_ne!(input.tokens[1], native.vocab().bos());
}

#[test]
fn gguf_raw_bos_follows_tokenizer_metadata() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::new().expect("backend");
    for add_bos in [false, true] {
        let mut params = Box::pin(model_params(Device::Cpu).expect("cpu params"));
        params.as_mut().append_kv_override(
            c"tokenizer.ggml.add_bos_token",
            ParamOverrideValue::Bool(add_bos),
        );
        let native = LlamaModel::load_from_file(&engine.backend, fixture(), &params)
            .expect("overridden tokenizer");
        let input = validate_text_and_tokenize(&native, "hello", false).expect("raw input");
        assert_eq!(input.tokens.first() == Some(&native.vocab().bos()), add_bos);
    }
}

#[test]
fn gguf_unsupported_template_fails_explicitly() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    let model = engine
        .load(&fixture(), test_device(), Arc::new(AtomicBool::new(false)))
        .expect("model");
    let template =
        llama_cpp_2::model::LlamaChatTemplate::new("{{ unsupported_tool_renderer(tools) }}")
            .expect("template");
    let messages = [LlamaChatMessage::new("user".into(), "hello".into()).expect("message")];
    assert!(
        model
            .model
            .apply_chat_template(&template, &messages, true)
            .is_err()
    );
}

fn test_device() -> Device {
    match std::env::var("SEFAI_TEST_BACKEND").as_deref() {
        Ok("metal") => Device::Gpu {
            backend: GpuBackend::Metal,
            index: std::env::var("SEFAI_TEST_DEVICE")
                .expect("SEFAI_TEST_DEVICE is required")
                .parse()
                .expect("device index"),
        },
        Ok("cpu") | Err(_) => Device::Cpu,
        other => panic!("unsupported test backend: {other:?}"),
    }
}

#[test]
fn gguf_every_stop_token_halts_generation() {
    let _guard = BACKEND.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut engine = Engine::new().expect("backend");
    let model = engine
        .load(&fixture(), test_device(), Arc::new(AtomicBool::new(false)))
        .expect("model");
    let input = model.prepare_raw("hello").expect("prompt");
    let vocab = model.model.vocab();
    let stop_tokens: Vec<_> = vocab
        .tokens()
        .filter(|token| vocab.is_eog(*token))
        .collect();
    assert_eq!(stop_tokens.len(), 3);
    for token in stop_tokens {
        let result = model
            .generate_with_sampler(
                &input,
                Settings::default(),
                || false,
                |_| panic!("stop token emitted text"),
                || {
                    LlamaSampler::chain_simple([
                        LlamaSampler::logit_bias(
                            vocab.n_tokens(),
                            &[llama_cpp_2::token::logit_bias::LlamaLogitBias::new(
                                token, 1e9,
                            )],
                        ),
                        LlamaSampler::greedy(),
                    ])
                },
            )
            .expect("forced stop token");
        assert_eq!(result.stop, Stop::EndOfGeneration);
        assert_eq!(result.generated_tokens, 1);
    }
}
