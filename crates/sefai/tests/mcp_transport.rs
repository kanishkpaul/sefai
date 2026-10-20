//! Dedicated process/network qualification; requires an explicit Python fixture executable.
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    server: Option<Child>,
}
impl Fixture {
    fn new(mode: &str, http: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "sefai-mcp-transport-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("required fixture operation succeeds");
        let initialized = Command::new(env!("CARGO_BIN_EXE_sefai"))
            .args([
                "--workspace",
                root.to_str().expect("fixture path is Unicode"),
                "workspace",
                "init",
            ])
            .output()
            .expect("required fixture operation succeeds");
        assert!(initialized.status.success());
        let python = PathBuf::from(
            std::env::var_os("SEFAI_MCP_TEST_PYTHON")
                .expect("set SEFAI_MCP_TEST_PYTHON to a Python 3 executable"),
        )
        .canonicalize()
        .expect("required fixture operation succeeds");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mcp_server.py")
            .canonicalize()
            .expect("required fixture operation succeeds");
        let mut server = None;
        let transport = if http {
            let mut child = Command::new(&python)
                .args([
                    script.to_str().expect("fixture path is Unicode"),
                    "--http",
                    "--mode",
                    mode,
                ])
                .current_dir(&root)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("required fixture operation succeeds");
            let mut endpoint = String::new();
            BufReader::new(child.stdout.take().expect("fixture stdout is piped"))
                .read_line(&mut endpoint)
                .expect("required fixture operation succeeds");
            assert!(
                endpoint.starts_with("http://127.0.0.1:"),
                "HTTP fixture failed to bind a loopback port"
            );
            server = Some(child);
            if mode.starts_with("oauth") {
                let port = std::net::TcpListener::bind("127.0.0.1:0")
                    .expect("required fixture operation succeeds")
                    .local_addr()
                    .expect("required fixture operation succeeds")
                    .port();
                format!(
                    "kind: http\n  url: {}\n  oauth:\n    issuer: {}\n    client_id: fixture-client\n    redirect_uri: http://127.0.0.1:{port}/callback\n    scopes: [tools]",
                    endpoint.trim(),
                    endpoint
                        .trim()
                        .strip_suffix("/mcp")
                        .expect("fixture endpoint ends with /mcp")
                )
            } else {
                format!(
                    "kind: http\n  url: {}\n  token_env: SEFAI_FIXTURE_AUTH",
                    endpoint.trim()
                )
            }
        } else {
            format!(
                "kind: stdio\n  command: {}\n  args: {}",
                json!(python),
                json!([script, "--mode", mode])
            )
        };
        fs::write(
            root.join("mcp/fixture.md"),
            format!("---\nschema: 1\nid: fixture\ntransport:\n  {transport}\n---\n"),
        )
        .expect("required fixture operation succeeds");
        Self { root, server }
    }
    fn run(&self, operation: &[&str], decision: &str, token: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
            .args([
                "--workspace",
                self.root.to_str().expect("fixture path is Unicode"),
                "mcp",
                "fixture",
            ])
            .args(operation)
            .env("SEFAI_FIXTURE_AUTH", token)
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("required fixture operation succeeds");
        child
            .stdin
            .take()
            .expect("required fixture operation succeeds")
            .write_all(decision.as_bytes())
            .expect("required fixture operation succeeds");
        child
            .wait_with_output()
            .expect("required fixture operation succeeds")
    }
    fn inspect(&self) -> Output {
        self.run(&["inspect"], "y\n", "fixture-only-token")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(server) = &mut self.server {
            let _ = server.kill();
            let _ = server.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn result(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("required fixture operation succeeds")
}
#[test]
fn stdio_discovers_paginated_catalog() {
    let fixture = Fixture::new("normal", false);
    assert_eq!(
        result(fixture.inspect())["tools"]
            .as_array()
            .expect("required fixture operation succeeds")
            .len(),
        2
    );
}
#[test]
fn stdio_denied_connection_does_not_start_server() {
    let fixture = Fixture::new("normal", false);
    assert!(
        !fixture
            .run(&["inspect"], "\n", "fixture-only-token")
            .status
            .success()
    );
    assert!(!fixture.root.join("fixture-started.txt").exists());
}
#[test]
fn stdio_rejects_unsupported_protocol() {
    assert!(!Fixture::new("version", false).inspect().status.success());
}
#[test]
fn stdio_rejects_duplicate_names() {
    assert!(!Fixture::new("duplicate", false).inspect().status.success());
}
#[test]
fn stdio_rejects_oversized_messages() {
    assert!(!Fixture::new("oversized", false).inspect().status.success());
}
#[test]
fn stdio_rejects_malformed_messages() {
    assert!(!Fixture::new("malformed", false).inspect().status.success());
}
#[test]
fn stdio_reads_only_a_selected_resource() {
    let fixture = Fixture::new("normal", false);
    assert_eq!(
        result(fixture.run(&["read", "fixture://note"], "y\n", "fixture-only-token"))["contents"]
            [0]["text"],
        "untrusted fixture resource"
    );
}
#[test]
fn stdio_retrieves_a_selected_prompt() {
    let fixture = Fixture::new("normal", false);
    assert_eq!(
        result(fixture.run(&["prompt", "greet"], "y\n", "fixture-only-token"))["messages"][0]["content"]
            ["text"],
        "untrusted fixture prompt"
    );
}
#[test]
fn http_requires_the_endpoint_credential() {
    let fixture = Fixture::new("normal", true);
    assert!(
        !fixture
            .run(&["inspect"], "y\n", "wrong-token")
            .status
            .success()
    );
}
#[test]
fn http_authenticates_and_discovers_tools() {
    let fixture = Fixture::new("normal", true);
    assert_eq!(
        result(fixture.inspect())["tools"]
            .as_array()
            .expect("required fixture operation succeeds")
            .len(),
        2
    );
}
#[test]
fn http_accepts_bounded_sse_post_responses() {
    let fixture = Fixture::new("sse", true);
    assert_eq!(
        result(fixture.inspect())["tools"]
            .as_array()
            .expect("required fixture operation succeeds")
            .len(),
        2
    );
}
#[test]
fn http_rejects_oversized_bodies() {
    assert!(!Fixture::new("oversized", true).inspect().status.success());
}
#[test]
fn http_rejects_malformed_messages() {
    assert!(!Fixture::new("malformed", true).inspect().status.success());
}
#[test]
fn http_does_not_follow_redirects() {
    assert!(!Fixture::new("redirect", true).inspect().status.success());
}

fn oauth_flow(mode: &str) -> (Output, bool) {
    let fixture = Fixture::new(mode, true);
    let mut child = Command::new(env!("CARGO_BIN_EXE_sefai"))
        .args([
            "--workspace",
            fixture.root.to_str().expect("fixture path is Unicode"),
            "mcp",
            "fixture",
            "inspect",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("required fixture operation succeeds");
    child
        .stdin
        .take()
        .expect("fixture stdin is piped")
        .write_all(b"y\n")
        .expect("required fixture operation succeeds");
    let mut reader = BufReader::new(child.stderr.take().expect("fixture stderr is piped"));
    let mut captured = String::new();
    let authorization = loop {
        let mut line = String::new();
        assert!(
            reader
                .read_line(&mut line)
                .expect("required fixture operation succeeds")
                > 0,
            "OAuth did not produce an authorization URL: {captured}"
        );
        captured.push_str(&line);
        if line.starts_with("http://127.0.0.1:") {
            break line;
        }
    };
    let python =
        std::env::var_os("SEFAI_MCP_TEST_PYTHON").expect("required fixture operation succeeds");
    let browser = Command::new(python)
        .args([
            "-c",
            "import sys, urllib.request; urllib.request.urlopen(sys.argv[1], timeout=10).read()",
            authorization.trim(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("required fixture operation succeeds");
    use std::io::Read;
    reader
        .read_to_string(&mut captured)
        .expect("required fixture operation succeeds");
    let mut output = child
        .wait_with_output()
        .expect("required fixture operation succeeds");
    output.stderr = captured.into_bytes();
    if mode == "oauth" {
        assert!(
            browser.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    (
        output,
        fixture.root.join("fixture-oauth-verified.txt").exists(),
    )
}

#[test]
fn oauth_verifies_pkce_resource_and_callback_state() {
    let (output, verified) = oauth_flow("oauth");
    assert_eq!(
        result(output)["tools"]
            .as_array()
            .expect("required fixture operation succeeds")
            .len(),
        2
    );
    assert!(verified);
}

#[test]
fn oauth_rejects_a_callback_with_wrong_state() {
    let (output, verified) = oauth_flow("oauth-bad-state");
    assert!(!output.status.success());
    assert!(!verified, "wrong state reached the token endpoint");
}

#[test]
fn oauth_rejects_metadata_for_a_different_issuer() {
    assert!(
        !Fixture::new("oauth-wrong-issuer", true)
            .inspect()
            .status
            .success()
    );
}

#[test]
fn oauth_rejects_an_unapproved_metadata_origin() {
    assert!(
        !Fixture::new("oauth-untrusted-origin", true)
            .inspect()
            .status
            .success()
    );
}

#[cfg(unix)]
#[test]
fn stdio_cleanup_stops_a_server_descendant() {
    let fixture = Fixture::new("child", false);
    assert!(fixture.inspect().status.success());
    let pid = fs::read_to_string(fixture.root.join("fixture-child.txt"))
        .expect("required fixture operation succeeds");
    let output = Command::new(std::env::var_os("SEFAI_MCP_TEST_PYTHON").expect("required fixture operation succeeds")).args(["-c", "import os, pathlib, sys; p=int(sys.argv[1]); state=pathlib.Path('/proc')/str(p)/'stat';\ntry:\n os.kill(p,0)\nexcept ProcessLookupError:\n sys.exit(0)\nif state.exists() and state.read_text().split(') ',1)[1].startswith('Z '): sys.exit(0)\nsys.exit(1)", &pid]).status().expect("required fixture operation succeeds");
    assert!(
        output.success(),
        "fixture descendant {pid} is still running"
    );
}

#[test]
fn stdio_initialization_disconnect_fails_without_retry() {
    assert!(
        !Fixture::new("disconnect-init", false)
            .inspect()
            .status
            .success()
    );
}
