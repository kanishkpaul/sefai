#![cfg(feature = "inference")]
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "sefai-chat-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(path)
    }
    fn chat(&self, input: &str) -> std::process::Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
            .args([
                "--workspace",
                self.0.to_str().expect("temp path"),
                "chat",
                "--plain",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("chat");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("input");
        child.wait_with_output().expect("exit")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn plain_first_run_and_help_have_no_terminal_controls() {
    let fixture = Fixture::new();
    let output = fixture.chat("/help\n/quit\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("/resume ID"));
    assert!(!stdout.contains('\x1b'));
    assert!(fixture.0.join("config.md").is_file());
}

#[test]
fn missing_model_is_recoverable_in_the_same_conversation() {
    let fixture = Fixture::new();
    let output = fixture.chat("/model /missing-sefai-model.gguf\nhello\n/help\n/quit\n");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("error:"));
    assert!(stdout.contains("/resume ID"));
}

#[test]
fn first_message_explains_how_to_choose_a_model() {
    let fixture = Fixture::new();
    let output = fixture.chat("hello\n/quit\n");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("choose /model PATH"));
}
