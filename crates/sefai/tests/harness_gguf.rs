//! Dedicated local-model gate; the fixture is required, never silently skipped.
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

static MODEL: Mutex<()> = Mutex::new(());
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let model = PathBuf::from(
            std::env::var_os("SEFAI_TOOL_TEST_MODEL")
                .expect("SEFAI_TOOL_TEST_MODEL must name the pinned tool-capable GGUF"),
        );
        assert!(model.is_file(), "required GGUF fixture is missing");
        let root = Self(std::env::temp_dir().join(format!(
            "sefai-tool-gate-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&root.0).expect("create unique fixture directory");
        fs::create_dir(root.0.join("models")).expect("create model directory");
        fs::create_dir(root.0.join("memory")).expect("create memory directory");
        fs::write(
            root.0.join("config.md"),
            "---\nschema: 1\nmodel: tools\n---\n",
        )
        .expect("write fixture configuration");
        fs::write(root.0.join("models/tools.md"), format!("---\nschema: 1\nid: tools\nformat: gguf\npath: '{}'\nrunner:\n  backend: cpu\n  context: 2048\n  output: 256\n  threads: 2\n---\n", model.display().to_string().replace('\'', "''"))).expect("write fixture profile");
        root
    }

    fn request(&self, query: &str, decision: &str) -> (String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
            .arg("--workspace")
            .arg(&self.0)
            .args(["chat", "--plain", "--tool-protocol", "smollm2"])
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn chat process");
        child
            .stdin
            .take()
            .expect("chat has piped stdin")
            .write_all(format!("{query}\n{decision}\n/quit\n").as_bytes())
            .expect("write user request and approval");
        let output = child.wait_with_output().expect("wait for chat exit");
        let text = String::from_utf8(output.stdout).expect("chat output is UTF-8");
        assert!(
            output.status.success(),
            "{text}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains("[approve?]"),
            "model did not propose a tool: {text}"
        );
        let session = fs::read_dir(self.0.join("sessions"))
            .expect("list session directory")
            .next()
            .expect("chat saved one session")
            .expect("read session directory entry")
            .path();
        let saved = fs::read_to_string(session).expect("read saved session");
        println!("{text}\n{saved}");
        (text, saved)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_gguf_reads_approved_memory_and_continues() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = Fixture::new();
    fs::write(
        root.0.join("memory/note.md"),
        "---\nschema: 1\nid: note\n---\nThe project marker is cobalt.\n",
    )
    .unwrap();
    let (text, saved) = root.request(
        "Read memory note using read_memory with id note, then tell me the project marker.",
        "y",
    );
    assert!(
        text.contains("read_memory id=note · completed"),
        "approved read did not finish: {text}"
    );
    assert!(
        text.rsplit("[assistant]")
            .next()
            .unwrap()
            .to_lowercase()
            .contains("cobalt"),
        "continuation did not use actual result: {text}"
    );
    assert!(saved.contains("## harness\n"));
    assert!(saved.contains("outcome: completed"));
    assert!(!text.contains("error:"), "{text}");
}

#[test]
fn real_gguf_creates_only_the_approved_memory() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = Fixture::new();
    let (text, saved) = root.request(
        "Call remember_memory with id rule and text Keep answers concise. Then confirm the result.",
        "y",
    );
    assert!(
        text.contains("remember_memory id=rule · completed"),
        "{text}"
    );
    let memory = fs::read_to_string(root.0.join("memory/rule.md")).unwrap();
    let preview: serde_json::Value = serde_json::from_str(
        text.lines()
            .find_map(|line| line.strip_prefix("arguments (Unicode escaped): "))
            .expect("write has an exact approval preview"),
    )
    .expect("ASCII fixture arguments are JSON");
    assert_eq!(preview["id"], "rule");
    let approved = preview["text"].as_str().expect("approved text is a string");
    assert_eq!(
        approved.strip_suffix('.').unwrap_or(approved),
        "Keep answers concise"
    );
    assert_eq!(memory, format!("---\nschema: 1\nid: rule\n---\n{approved}"));
    let dispatch = saved
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.strip_prefix("> ")?).ok())
        .find(|record| record["outcome"] == "dispatching")
        .expect("write has a durable dispatch intent");
    assert_eq!(dispatch["tool"], "remember_memory");
    assert_eq!(dispatch["arguments"], preview);
    assert_eq!(fs::read_dir(root.0.join("memory")).unwrap().count(), 1);
    assert_eq!(saved.matches("\"outcome\":\"dispatching\"").count(), 1);
    assert!(
        !text.contains("error:")
            || text.contains("error: tool call already attempted; automatic retry refused"),
        "{text}"
    );
}

#[test]
fn real_gguf_denied_write_creates_no_memory() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = Fixture::new();
    let (text, saved) = root.request(
        "Call remember_memory with id rule and text Keep answers concise. Then confirm the result.",
        "n",
    );
    assert!(text.contains("remember_memory id=rule · denied"), "{text}");
    assert_eq!(fs::read_dir(root.0.join("memory")).unwrap().count(), 0);
    assert!(saved.contains("\"outcome\":\"denied\""));
    assert!(!saved.contains("\"outcome\":\"dispatching\""));
    assert!(
        !text.contains("error:")
            || text.contains("error: tool call already attempted; automatic retry refused"),
        "{text}"
    );
}
