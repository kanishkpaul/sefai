//! Killing sefai mid-reply must leave the workspace valid and the session resumable; the real
//! GGUF fixture is mandatory.
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn a_killed_reply_leaves_a_valid_resumable_workspace() {
    let model =
        PathBuf::from(std::env::var_os("SEFAI_LIFECYCLE_GGUF").expect("set SEFAI_LIFECYCLE_GGUF"));
    assert!(model.is_file(), "required model fixture is missing");
    let root = std::env::temp_dir().join(format!("sefai-recovery-{}", std::process::id()));
    fs::create_dir_all(root.join("models")).expect("models");
    fs::write(
        root.join("config.md"),
        "---\nschema: 1\nmodel: small\n---\n",
    )
    .expect("config");
    fs::write(
        root.join("models/small.md"),
        format!(
            "---\nschema: 1\nid: small\nformat: gguf\npath: '{}'\nrunner:\n  output: 2048\n---\n",
            model.display().to_string().replace('\'', "''")
        ),
    )
    .expect("profile");
    let sefai = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_sefai"))
            .arg("--workspace")
            .arg(&root)
            .args(arguments)
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sefai")
    };

    // First session: one completed turn, then a long reply killed while streaming.
    let mut child = sefai(&["chat", "--plain"]);
    let mut input = child.stdin.take().expect("stdin");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    input.write_all(b"Say one word.\n").expect("first turn");
    let mut line = String::new();
    let mut ready = 0;
    while ready < 2 {
        line.clear();
        assert!(
            output.read_line(&mut line).expect("read") > 0,
            "chat ended early"
        );
        ready += line.matches("[ready] ").count();
    }
    input
        .write_all(b"Write a very long story about rivers, at least two thousand words.\n")
        .expect("second turn");
    loop {
        line.clear();
        assert!(
            output.read_line(&mut line).expect("read") > 0,
            "chat ended early"
        );
        if line.contains("[assistant]") {
            break;
        }
    }
    child.kill().expect("kill mid-reply");
    child.wait().expect("reap");

    // The workspace still validates, and no temporary file became a record.
    let check = Command::new(env!("CARGO_BIN_EXE_sefai"))
        .arg("--workspace")
        .arg(&root)
        .args(["workspace", "check"])
        .output()
        .expect("check");
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let sessions = fs::read_dir(root.join("sessions"))
        .expect("sessions")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect::<Vec<_>>();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    let id = sessions[0]
        .file_stem()
        .expect("stem")
        .to_string_lossy()
        .into_owned();

    // The session resumes with the completed turn and accepts a new one.
    let resumed = sefai(&["--session", &id, "chat", "--plain"]);
    let mut resumed_input = resumed.stdin.as_ref().expect("stdin");
    resumed_input
        .write_all(b"Say one word.\n/quit\n")
        .expect("resume turn");
    let result = resumed.wait_with_output().expect("resume");
    let text = String::from_utf8_lossy(&result.stdout).into_owned();
    let _ = fs::remove_dir_all(&root);
    assert!(result.status.success(), "{text}");
    assert!(text.contains("[user]\nSay one word."), "{text}");
    assert!(!text.contains("error:"), "{text}");
}
