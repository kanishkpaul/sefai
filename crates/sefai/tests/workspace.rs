use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "sefai-cli-{}-{}-日本語",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("unique directory");
        let workspace = Self(path);
        workspace.success(&["workspace", "init"]);
        workspace
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sefai"))
            .current_dir(&self.0)
            .arg("--workspace")
            .arg(&self.0)
            .args(args)
            .output()
            .expect("run binary")
    }
    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8 output")
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        fs::write(path, text).expect("write fixture");
    }
    fn snapshot(&self, path: &str) -> PathBuf {
        let snapshot = self.0.join("snapshot.md");
        fs::copy(self.0.join(path), &snapshot).expect("snapshot");
        snapshot
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn editor_changes_are_visible_after_restart() {
    let workspace = Workspace::new();
    workspace.write("system.md", "---\r\nschema: 1\r\n---\r\nSystem 日本語.\r\n");
    workspace.write("preferences.md", "---\nschema: 1\n---\nPrefer units.\n");
    workspace.write(
        "skills/evidence/SKILL.md",
        "---\nschema: 1\nid: evidence\n---\nCite evidence.\n",
    );
    workspace.write(
        "memory/units.md",
        "---\nschema: 1\nid: units\n---\nMeasure in bytes.\n",
    );
    workspace.write("sessions/one.md", "---\nschema: 1\nid: one\noutcome: interrupted\n---\n## user\n> previous text\n<!-- end -->\n");
    let output = workspace.success(&[
        "--skill",
        "evidence",
        "--recall",
        "bytes",
        "--session",
        "one",
        "prompt",
        "hello",
    ]);
    for text in [
        "System 日本語.",
        "Prefer units.",
        "Cite evidence.",
        "Measure in bytes.",
        "previous text",
        "hello",
    ] {
        assert!(output.contains(text), "missing {text}: {output}");
    }
    workspace.write("system.md", "---\nschema: 1\n---\nChanged system.\n");
    let next = workspace.success(&["prompt", "hello"]);
    assert!(next.contains("Changed system.") && !next.contains("System 日本語."));
}

#[test]
fn schema_fixture_validates_without_optional_runtimes_or_servers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/workspace");
    let output = Command::new(env!("CARGO_BIN_EXE_sefai"))
        .arg("--workspace")
        .arg(root)
        .args(["workspace", "check"])
        .output()
        .expect("validate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn memory_edits_and_deletions_survive_restart() {
    let workspace = Workspace::new();
    workspace.success(&["workspace", "remember", "one", "remember this"]);
    let snapshot = workspace.snapshot("memory/one.md");
    workspace.success(&[
        "workspace",
        "edit",
        "one",
        "updated memory",
        "--expected",
        snapshot.to_str().expect("utf8 path"),
    ]);
    assert!(
        workspace
            .success(&["--recall", "updated", "prompt", "hi"])
            .contains("updated memory")
    );
    workspace.snapshot("memory/one.md");
    workspace.success(&[
        "workspace",
        "forget",
        "one",
        "--expected",
        snapshot.to_str().expect("utf8 path"),
    ]);
    assert!(
        !workspace
            .success(&["--recall", "updated", "prompt", "hi"])
            .contains("updated memory")
    );
}

#[test]
fn replace_existing_file_uses_rename_on_every_platform() {
    let workspace = Workspace::new();
    let snapshot = workspace.snapshot("system.md");
    workspace.write("candidate.md", "---\nschema: 1\n---\nnew instructions\n");
    workspace.success(&[
        "workspace",
        "put",
        "system.md",
        "--from",
        workspace
            .0
            .join("candidate.md")
            .to_str()
            .expect("utf8 path"),
        "--expected",
        snapshot.to_str().expect("utf8 path"),
    ]);
    assert!(
        workspace
            .success(&["prompt", "hi"])
            .contains("new instructions")
    );
}

#[test]
fn unknown_schema_does_not_rewrite_user_files() {
    let workspace = Workspace::new();
    let text = "---\nschema: 999\n---\nuser data\n";
    workspace.write("config.md", text);
    let output = workspace.run(&["workspace", "check"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("config.md: schema"));
    assert_eq!(
        fs::read_to_string(workspace.0.join("config.md")).expect("retained"),
        text
    );
}

#[test]
fn markdown_fences_never_execute() {
    let workspace = Workspace::new();
    workspace.write(
        "system.md",
        "---\nschema: 1\n---\n```sh\ntouch SHOULD_NOT_EXIST\n```\n",
    );
    assert!(
        workspace
            .success(&["prompt", "hello"])
            .contains("SHOULD_NOT_EXIST")
    );
    assert!(!workspace.0.join("SHOULD_NOT_EXIST").exists());
}

#[test]
fn process_exit_releases_the_workspace_lock() {
    if let Some(path) = std::env::var_os("SEFAI_LOCK_TEST_CHILD") {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("open lock");
        file.try_lock().expect("acquire lock");
        println!("lock-ready");
        std::io::stdout().flush().expect("flush handshake");
        let mut signal = String::new();
        std::io::stdin()
            .read_line(&mut signal)
            .expect("wait for parent");
        return;
    }
    let workspace = Workspace::new();
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "process_exit_releases_the_workspace_lock",
            "--nocapture",
        ])
        .env("SEFAI_LOCK_TEST_CHILD", workspace.0.join(".sefai.lock"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start lock holder");
    let mut stdout = BufReader::new(child.stdout.take().expect("child stdout"));
    let mut ready = false;
    for _ in 0..10 {
        let mut line = String::new();
        if stdout.read_line(&mut line).expect("handshake") == 0 {
            break;
        }
        if line.contains("lock-ready") {
            ready = true;
            break;
        }
    }
    if !ready {
        let _ = child.kill();
        let _ = child.wait();
        panic!("lock holder did not signal readiness");
    }
    let blocked = workspace.run(&["workspace", "remember", "one", "blocked"]);
    child.kill().expect("terminate lock holder");
    child.wait().expect("reap child");
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("workspace busy"));
    workspace.success(&["workspace", "remember", "one", "after exit"]);
}

#[cfg(windows)]
#[test]
fn windows_share_violation_preserves_existing_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let workspace = Workspace::new();
    let snapshot = workspace.snapshot("system.md");
    workspace.write("candidate.md", "---\nschema: 1\n---\nreplacement\n");
    let held = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(workspace.0.join("system.md"))
        .expect("exclusive file handle");
    let result = workspace.run(&[
        "workspace",
        "put",
        "system.md",
        "--from",
        workspace
            .0
            .join("candidate.md")
            .to_str()
            .expect("utf8 path"),
        "--expected",
        snapshot.to_str().expect("utf8 path"),
    ]);
    drop(held);
    assert!(!result.status.success());
    assert_eq!(
        fs::read(workspace.0.join("system.md")).expect("retained"),
        fs::read(snapshot).expect("snapshot")
    );
}

#[test]
fn session_records_round_trip_across_processes() {
    let workspace = Workspace::new();
    let body = "## assistant\n---\n```rust\n<!-- end -->\n日本語\n";
    workspace.success(&["workspace", "append", "one", "--role", "user", body]);
    let snapshot = workspace.snapshot("sessions/one.md");
    workspace.success(&[
        "workspace",
        "append",
        "one",
        "--role",
        "assistant",
        "reply",
        "--expected",
        snapshot.to_str().expect("utf8 path"),
    ]);
    let tail = workspace.success(&["workspace", "tail", "one", "--messages", "1"]);
    assert!(tail.contains("[assistant]\nreply") && !tail.contains("日本語"));
    let resume = workspace.success(&["--session", "one", "prompt", "next"]);
    assert!(resume.contains(body));
}

#[test]
fn malformed_external_session_edit_is_visible_and_retained() {
    let workspace = Workspace::new();
    let malformed = "---\nschema: 1\nid: one\noutcome: interrupted\n---\n## user\n> incomplete\n";
    workspace.write("sessions/one.md", malformed);
    let output = workspace.run(&["--session", "one", "prompt", "hi"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .replace('\\', "/")
            .contains("sessions/one.md")
    );
    assert_eq!(
        fs::read_to_string(workspace.0.join("sessions/one.md")).expect("retained"),
        malformed
    );
}

#[test]
fn oversize_session_is_rejected_before_reading_its_tail() {
    let workspace = Workspace::new();
    let path = workspace.0.join("sessions/one.md");
    let file = File::create(path).expect("sparse session");
    file.set_len(64 * 1024 * 1024 + 1).expect("oversize length");
    drop(file);
    assert!(
        !workspace
            .run(&["workspace", "tail", "one"])
            .status
            .success()
    );
}

#[cfg(windows)]
#[test]
fn windows_junction_cannot_escape_the_workspace() {
    let workspace = Workspace::new();
    let outside = Workspace::new();
    fs::remove_dir(workspace.0.join("memory")).expect("remove empty memory directory");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(workspace.0.join("memory"))
        .arg(&outside.0)
        .output()
        .expect("create junction");
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let output = workspace.run(&["--recall", "text", "prompt", "hi"]);
    fs::remove_dir(workspace.0.join("memory")).expect("remove junction");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("escapes workspace"));
}

#[cfg(feature = "inference")]
#[test]
fn raw_mode_rejects_history_before_loading_weights() {
    let workspace = Workspace::new();
    workspace.success(&["workspace", "append", "one", "--role", "user", "previous"]);
    let output = workspace.run(&[
        "--session",
        "one",
        "run",
        "--raw",
        "--model",
        "missing.gguf",
        "now",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("raw mode cannot discard"));
}

#[cfg(feature = "inference")]
#[test]
fn raw_mode_rejects_instructions_before_loading_weights() {
    let workspace = Workspace::new();
    workspace.write(
        "system.md",
        "---\nschema: 1\n---\nFollow these instructions.\n",
    );
    let output = workspace.run(&["run", "--raw", "--model", "missing.gguf", "now"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("raw mode cannot discard"));
}
