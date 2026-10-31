//! The lifecycle every qualified format shares: profile selection, Unicode streaming, context
//! rejection before streaming, admission failure and recovery, session resume and statistics.
//! Engine-specific behavior stays in each adapter's own tests.

use super::{FORMAT, MODEL_ENV};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

static MODEL: Mutex<()> = Mutex::new(());
static NEXT: AtomicUsize = AtomicUsize::new(0);
const UNICODE: &str = "café 東京 🙂";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let model = PathBuf::from(
            std::env::var_os(MODEL_ENV)
                .expect("set the model environment variable to the pinned fixture"),
        );
        assert!(model.exists(), "required model fixture is missing");
        let root = Self(std::env::temp_dir().join(format!(
            "sefai-lifecycle-{FORMAT}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir_all(root.0.join("models")).expect("create model directory");
        fs::write(
            root.0.join("config.md"),
            "---\nschema: 1\nmodel: good\n---\n",
        )
        .expect("write configuration");
        fs::write(
            root.0.join("system.md"),
            "---\nschema: 1\n---\nAnswer briefly.\n",
        )
        .expect("write system prompt");
        root.profile("good", &model, 2048);
        root.profile("small", &model, 512);
        // Every engine rejects a Markdown file as its model during admission.
        root.profile("broken", &root.0.join("system.md"), 2048);
        root
    }

    fn profile(&self, id: &str, path: &std::path::Path, context: u32) {
        let path = path.display().to_string().replace('\'', "''");
        fs::write(
            self.0.join(format!("models/{id}.md")),
            format!(
                "---\nschema: 1\nid: {id}\nformat: {FORMAT}\npath: '{path}'\nrunner:\n  context: {context}\n  output: 32\n---\n"
            ),
        )
        .expect("write profile");
    }

    fn sefai(&self, arguments: &[&str], input: &str) -> (Output, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
            .arg("--workspace")
            .arg(&self.0)
            .args(arguments)
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sefai");
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input.as_bytes())
            .expect("write input");
        let output = child.wait_with_output().expect("wait for sefai");
        let text = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
        println!("{text}\n{}", String::from_utf8_lossy(&output.stderr));
        (output, text)
    }

    fn session(&self) -> String {
        let mut sessions = fs::read_dir(self.0.join("sessions"))
            .expect("list sessions")
            .map(|entry| entry.expect("session entry").path())
            .collect::<Vec<_>>();
        assert_eq!(sessions.len(), 1, "expected one saved session");
        sessions
            .pop()
            .and_then(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .expect("session file name")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn reply(text: &str) -> &str {
    text.rsplit("[assistant]\n")
        .next()
        .expect("assistant label")
}

#[test]
fn profile_streams_unicode_through_the_terminal_worker() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (output, text) = fixture.sefai(
        &["chat", "--plain"],
        &format!("Repeat these words: {UNICODE}\n/quit\n"),
    );
    assert!(output.status.success());
    assert!(
        text.contains(&format!("[user]\nRepeat these words: {UNICODE}")),
        "{text}"
    );
    assert!(!reply(&text).trim().is_empty(), "empty reply: {text}");
    assert!(!text.contains("error:"), "{text}");
}

#[test]
fn context_overflow_is_rejected_before_streaming() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let long = "Describe the river and its stones. ".repeat(100);
    let (output, text) = fixture.sefai(
        &["--profile", "small", "chat", "--plain"],
        &format!("{long}\n/quit\n"),
    );
    assert!(output.status.success());
    assert!(text.contains("exceeds context"), "{text}");
    assert!(!text.contains("[assistant]"), "{text}");
}

#[test]
fn failed_admission_leaves_the_session_usable() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (output, text) = fixture.sefai(
        &["chat", "--plain"],
        "/profile broken\nhello\n/profile good\nReply with one short greeting.\n/quit\n",
    );
    assert!(output.status.success());
    assert!(text.contains("error: unsupported artifact:"), "{text}");
    assert!(
        text.contains("[user]\nReply with one short greeting."),
        "{text}"
    );
    assert!(!reply(&text).trim().is_empty(), "empty reply: {text}");
}

#[test]
fn resumed_session_continues_after_restart() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (first, _) = fixture.sefai(&["chat", "--plain"], "Remember the word lantern.\n/quit\n");
    assert!(first.status.success());
    let id = fixture.session();
    let (second, text) = fixture.sefai(
        &["--session", &id, "chat", "--plain"],
        "Reply with one short greeting.\n/quit\n",
    );
    assert!(second.status.success());
    assert!(
        text.contains("[user]\nRemember the word lantern."),
        "history was not resumed: {text}"
    );
    assert!(
        text.contains("[user]\nReply with one short greeting."),
        "{text}"
    );
    assert!(!text.contains("error:"), "{text}");
}

#[test]
fn bench_reports_statistics_and_engine_memory() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (output, text) = fixture.sefai(&["bench", "--repetitions", "1", "hello"], "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        text.contains("sample=1 ") && text.contains("generated_tokens="),
        "{text}"
    );
    assert!(
        stderr.contains("memory") || stderr.contains("mlx_active_bytes="),
        "{stderr}"
    );
}

#[test]
fn run_reports_the_selected_engine_device() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (output, _) = fixture.sefai(&["run", "--output", "4", "hello"], "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let expected = match FORMAT {
        "gguf" => "local · cpu ·",
        "onnx" => "local · onnx cpu ·",
        _ => "local · mlx apple gpu ·",
    };
    assert!(stderr.contains(expected), "{stderr}");
}
