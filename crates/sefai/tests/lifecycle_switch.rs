//! One terminal session switches between GGUF, MLX and ONNX profiles, unloading each engine's
//! model before the next loads; every fixture is mandatory.
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn one_session_switches_between_engines() {
    let root = std::env::temp_dir().join(format!("sefai-lifecycle-switch-{}", std::process::id()));
    fs::create_dir_all(root.join("models")).expect("create model directory");
    fs::write(root.join("config.md"), "---\nschema: 1\nmodel: gguf\n---\n")
        .expect("write configuration");
    for (format, variable) in [
        ("gguf", "SEFAI_LIFECYCLE_GGUF"),
        ("mlx", "SEFAI_LIFECYCLE_MLX"),
        ("onnx", "SEFAI_LIFECYCLE_ONNX"),
    ] {
        let model = PathBuf::from(
            std::env::var_os(variable).expect("set every lifecycle model environment variable"),
        );
        assert!(model.exists(), "required {format} fixture is missing");
        let path = model.display().to_string().replace('\'', "''");
        fs::write(
            root.join(format!("models/{format}.md")),
            format!("---\nschema: 1\nid: {format}\nformat: {format}\npath: '{path}'\nrunner:\n  output: 16\n---\n"),
        )
        .expect("write profile");
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
        .arg("--workspace")
        .arg(&root)
        .args(["chat", "--plain"])
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn chat");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"Say one.\n/profile mlx\nSay two.\n/profile onnx\nSay three.\n/profile gguf\nSay four.\n/quit\n")
        .expect("write input");
    let output = child.wait_with_output().expect("wait for chat");
    let _ = fs::remove_dir_all(&root);
    let text = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    println!("{text}");
    assert!(output.status.success());
    assert!(!text.contains("error:"), "{text}");
    // Plain mode prints the ready status directly after the streamed reply.
    let ready = text
        .split("[ready] ")
        .filter(|status| status.contains(" · ctx "))
        .map(|status| status.split(" · ").nth(1).unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ready.len(), 4, "{text}");
    assert_eq!(ready[0], "cpu");
    assert_eq!(ready[1], "mlx apple gpu");
    assert!(ready[2].starts_with("onnx cpu"), "{text}");
    assert_eq!(ready[3], "cpu");
}
