use super::{BACKEND, FORMAT, MODEL_ENV};
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
            std::env::var_os(MODEL_ENV)
                .expect("set the model environment variable to the pinned tool-capable fixture"),
        );
        assert!(
            if BACKEND == "mlx" {
                model.is_dir()
            } else {
                model.is_file()
            },
            "required model fixture is missing"
        );
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
        fs::write(root.0.join("models/tools.md"), format!("---\nschema: 1\nid: tools\nformat: {FORMAT}\npath: '{}'\nrunner:\n  backend: {BACKEND}\n  context: 2048\n  output: 256\n  threads: 2\n---\n", model.display().to_string().replace('\'', "''"))).expect("write fixture profile");
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
fn real_model_reads_approved_memory_and_continues() {
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
fn real_model_creates_only_the_approved_memory() {
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
fn real_model_denied_write_creates_no_memory() {
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

#[cfg(feature = "mcp")]
impl Fixture {
    fn request_mcp(&self, tool: &str, decision: &str, mode: &str) -> (String, String) {
        let python = PathBuf::from(
            std::env::var_os("SEFAI_MCP_TEST_PYTHON")
                .expect("set SEFAI_MCP_TEST_PYTHON for MCP model qualification"),
        )
        .canonicalize()
        .expect("required MCP model fixture operation succeeds");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mcp_server.py")
            .canonicalize()
            .expect("required MCP model fixture operation succeeds");
        fs::create_dir(self.0.join("mcp")).expect("required MCP model fixture operation succeeds");
        fs::write(self.0.join("mcp/fixture.md"), format!("---\nschema: 1\nid: fixture\ntransport:\n  kind: stdio\n  command: {}\n  args: {}\n---\n", serde_json::json!(python), serde_json::json!([script, "--mode", mode]))).expect("required MCP model fixture operation succeeds");
        let profile = self.0.join("models/tools.md");
        fs::write(
            &profile,
            fs::read_to_string(&profile)
                .expect("required MCP model fixture operation succeeds")
                .replace("context: 2048", "context: 4096"),
        )
        .expect("required MCP model fixture operation succeeds");
        let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
            .arg("--workspace")
            .arg(&self.0)
            .args([
                "--profile",
                "tools",
                "chat",
                "--plain",
                "--tool-protocol",
                "smollm2",
                "--mcp",
                "fixture",
            ])
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("required MCP model fixture operation succeeds");
        let query = format!(
            "Use the mcp_call function with id fixture and mcp {{\"tool\":\"{tool}\",\"arguments\":{{\"text\":\"cobalt\"}}}}. Then tell me the returned marker."
        );
        child
            .stdin
            .take()
            .expect("required MCP model fixture operation succeeds")
            .write_all(format!("y\n{query}\n{decision}\n/quit\n").as_bytes())
            .expect("required MCP model fixture operation succeeds");
        let output = child
            .wait_with_output()
            .expect("required MCP model fixture operation succeeds");
        let text = String::from_utf8(output.stdout)
            .expect("required MCP model fixture operation succeeds");
        assert!(
            output.status.success(),
            "{text}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let session = fs::read_dir(self.0.join("sessions"))
            .expect("required MCP model fixture operation succeeds")
            .next()
            .expect("required MCP model fixture operation succeeds")
            .expect("required MCP model fixture operation succeeds")
            .path();
        let saved =
            fs::read_to_string(session).expect("required MCP model fixture operation succeeds");
        println!("{text}\n{saved}");
        (text, saved)
    }
}

#[cfg(feature = "mcp")]
#[test]
fn real_model_uses_an_approved_mcp_result() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (text, saved) = fixture.request_mcp("echo", "y", "normal");
    assert!(text.contains("fixture/echo · completed"), "{text}");
    assert!(
        text.rsplit("[assistant]")
            .next()
            .unwrap()
            .contains("cobalt"),
        "{text}"
    );
    assert_eq!(saved.matches("\"outcome\":\"dispatching\"").count(), 1);
    assert!(!text.contains("error:"), "{text}");
}

#[cfg(feature = "mcp")]
#[test]
fn real_model_denied_mcp_write_has_no_side_effect() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (text, saved) = fixture.request_mcp("remember", "n", "normal");
    assert!(text.contains("fixture/remember · denied"), "{text}");
    assert!(!fixture.0.join("fixture-write.txt").exists());
    assert!(!saved.contains("\"outcome\":\"dispatching\""));
}

#[cfg(feature = "mcp")]
#[test]
fn real_model_mcp_write_matches_approved_arguments() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (text, saved) = fixture.request_mcp("remember", "y", "normal");
    assert!(text.contains("fixture/remember · completed"), "{text}");
    let preview: serde_json::Value = serde_json::from_str(
        text.lines()
            .find_map(|line| line.strip_prefix("arguments (Unicode escaped): "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(fixture.0.join("fixture-write.txt")).unwrap(),
        preview["text"].as_str().unwrap()
    );
    assert_eq!(saved.matches("\"outcome\":\"dispatching\"").count(), 1);
}

#[cfg(feature = "mcp")]
#[test]
fn real_model_mcp_disconnect_after_write_is_uncertain_and_not_replayed() {
    let _model = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::new();
    let (text, saved) = fixture.request_mcp("remember", "y", "disconnect");
    assert!(text.contains("fixture/remember · unknown"), "{text}");
    assert!(fixture.0.join("fixture-write.txt").exists());
    assert!(saved.contains("\"outcome\":\"unknown\""));
    assert_eq!(saved.matches("\"outcome\":\"dispatching\"").count(), 1);
}
