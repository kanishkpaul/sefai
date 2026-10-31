#![cfg(feature = "inference")]
//! `sefai setup` through the binary, without network access or a loaded model.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sefai-setup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let fixture = Self(root);
        let output = fixture.sefai(&["workspace", "init"]);
        assert!(output.status.success());
        fixture
    }

    fn sefai(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sefai"))
            .arg("--workspace")
            .arg(&self.0)
            .args(arguments)
            .env("NO_COLOR", "1")
            .output()
            .expect("run sefai")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr)
}

/// A GGUF header with a SmolLM2-135M attention shape and no tensors.
fn gguf(path: &Path) {
    let mut header = b"GGUF".to_vec();
    header.extend(3u32.to_le_bytes());
    header.extend(0u64.to_le_bytes());
    let entries: [(&str, u32); 5] = [
        ("llama.block_count", 30),
        ("llama.context_length", 8192),
        ("llama.embedding_length", 576),
        ("llama.attention.head_count", 9),
        ("llama.attention.head_count_kv", 3),
    ];
    header.extend((entries.len() as u64 + 1).to_le_bytes());
    for (key, value) in [("general.architecture", None)]
        .into_iter()
        .chain(entries.iter().map(|(key, value)| (*key, Some(*value))))
    {
        header.extend((key.len() as u64).to_le_bytes());
        header.extend(key.as_bytes());
        match value {
            None => {
                header.extend(8u32.to_le_bytes());
                header.extend(5u64.to_le_bytes());
                header.extend(b"llama");
            }
            Some(value) => {
                header.extend(4u32.to_le_bytes());
                header.extend(value.to_le_bytes());
            }
        }
    }
    fs::write(path, header).expect("write gguf header");
}

#[test]
fn catalog_lists_every_built_in_entry_with_its_pin() {
    let fixture = Fixture::new();
    let output = fixture.sefai(&["setup", "catalog"]);
    assert!(output.status.success(), "{}", text(&output));
    let listing = text(&output);
    assert_eq!(listing.lines().count(), 14, "{listing}");
    assert!(listing.contains("smollm2-135m-gguf · gguf · 135M parameters"));
}

#[test]
fn check_estimates_a_local_model_without_loading_it() {
    let fixture = Fixture::new();
    let model = fixture.0.join("header.gguf");
    gguf(&model);
    let output = fixture.sefai(&[
        "setup",
        "check",
        model.to_str().expect("utf-8 path"),
        "--format",
        "gguf",
    ]);
    assert!(output.status.success(), "{}", text(&output));
    let report = text(&output);
    assert!(
        report.contains("30 layers · 3 KV heads × 64 dimensions"),
        "{report}"
    );
    assert!(
        report.contains("KV cache 0.09 GiB at 4096 tokens"),
        "{report}"
    );
    assert!(report.contains("no weights were loaded"));
}

#[test]
fn context_beyond_the_trained_limit_is_rejected() {
    let fixture = Fixture::new();
    let model = fixture.0.join("header.gguf");
    gguf(&model);
    let output = fixture.sefai(&[
        "setup",
        "check",
        model.to_str().expect("utf-8 path"),
        "--format",
        "gguf",
        "--context",
        "16384",
    ]);
    assert!(!output.status.success());
    assert!(text(&output).contains("exceeds the model's trained context 8192"));
}

#[test]
fn import_writes_a_profile_once() {
    let fixture = Fixture::new();
    let model = fixture.0.join("header.gguf");
    gguf(&model);
    let path = model.to_str().expect("utf-8 path");
    let output = fixture.sefai(&["setup", "import", "small", path, "--format", "gguf"]);
    assert!(output.status.success(), "{}", text(&output));
    let profile = fs::read_to_string(fixture.0.join("models/small.md")).expect("profile");
    assert!(profile.contains("format: gguf"));
    assert!(fixture.sefai(&["workspace", "check"]).status.success());
    assert!(
        !fixture
            .sefai(&["setup", "import", "small", path, "--format", "gguf"])
            .status
            .success()
    );
}

#[test]
fn a_non_gguf_file_is_not_imported() {
    let fixture = Fixture::new();
    let path = fixture.0.join("system.md");
    let output = fixture.sefai(&[
        "setup",
        "import",
        "bad",
        path.to_str().expect("utf-8 path"),
        "--format",
        "gguf",
    ]);
    assert!(!output.status.success());
    assert!(!fixture.0.join("models/bad.md").exists());
}

#[test]
fn workspace_catalog_entries_join_the_built_ins() {
    let fixture = Fixture::new();
    let built_in = include_str!("../catalog/smollm2-135m-gguf.md");
    fs::create_dir_all(fixture.0.join("catalog")).expect("catalog directory");
    fs::write(
        fixture.0.join("catalog/my-small.md"),
        built_in.replace("id: smollm2-135m-gguf", "id: my-small"),
    )
    .expect("custom entry");
    let output = fixture.sefai(&["setup", "catalog"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("my-small · gguf"));
    assert!(fixture.sefai(&["workspace", "check"]).status.success());
}

#[test]
fn a_workspace_entry_cannot_reuse_a_built_in_id() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.0.join("catalog")).expect("catalog directory");
    fs::write(
        fixture.0.join("catalog/smollm2-135m-gguf.md"),
        include_str!("../catalog/smollm2-135m-gguf.md"),
    )
    .expect("duplicate entry");
    let output = fixture.sefai(&["setup", "catalog"]);
    assert!(!output.status.success());
    assert!(text(&output).contains("duplicates a built-in entry"));
}

#[test]
fn an_escaping_catalog_file_path_is_rejected() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.0.join("catalog")).expect("catalog directory");
    fs::write(
        fixture.0.join("catalog/escape.md"),
        include_str!("../catalog/smollm2-135m-gguf.md")
            .replace("id: smollm2-135m-gguf", "id: escape")
            .replace(
                "path: 'SmolLM2-135M-Instruct-Q4_K_M.gguf'",
                "path: '../outside.gguf'",
            )
            .replace(
                "entry: 'SmolLM2-135M-Instruct-Q4_K_M.gguf'",
                "entry: '../outside.gguf'",
            ),
    )
    .expect("escaping entry");
    assert!(!fixture.sefai(&["workspace", "check"]).status.success());
    assert!(!fixture.sefai(&["setup", "catalog"]).status.success());
}

#[test]
fn get_without_confirmation_downloads_nothing() {
    let fixture = Fixture::new();
    let output = fixture.sefai(&["setup", "get", "smollm2-135m-gguf"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(!fixture.0.join("weights").exists());
    assert!(!fixture.0.join("models/smollm2-135m-gguf.md").exists());
}
