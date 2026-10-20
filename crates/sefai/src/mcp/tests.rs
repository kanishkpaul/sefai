use super::*;
use std::{fs, sync::atomic::AtomicUsize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sefai-mcp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Workspace::init(&root).unwrap();
        Self(root)
    }
    fn definition(&self) -> Definition {
        let exe = std::env::current_exe().unwrap().canonicalize().unwrap();
        fs::write(
            self.0.join("mcp/fixture.md"),
            format!(
                "---\nschema: 1\nid: fixture\ntransport:\n  kind: stdio\n  command: {}\n---\n",
                serde_json::to_string(&exe).unwrap()
            ),
        )
        .unwrap();
        Definition::load(&self.0, "fixture").unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_connection(fixture: &Fixture, responses: Vec<Value>) -> Connection {
    fixture.definition();
    connection_at(&fixture.0, responses).0
}

pub(crate) fn connection_at(
    root: &Path,
    responses: Vec<Value>,
) -> (Connection, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
    if !root.join("mcp/fixture.md").exists() {
        let exe = std::env::current_exe().unwrap().canonicalize().unwrap();
        fs::write(
            root.join("mcp/fixture.md"),
            format!(
                "---\nschema: 1\nid: fixture\ntransport:\n  kind: stdio\n  command: {}\n---\n",
                serde_json::to_string(&exe).unwrap()
            ),
        )
        .unwrap();
    }
    let definition = Definition::load(root, "fixture").unwrap();
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let received = captured.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(MESSAGE_BYTES_MAX * 2);
    let service = runtime.block_on(async {
        tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(server);
            let mut lines = BufReader::new(read).lines();
            for result in responses {
                loop {
                    let Some(line) = lines.next_line().await.unwrap() else {
                        return;
                    };
                    let request: Value = serde_json::from_str(&line).unwrap();
                    received.lock().unwrap().push(request.clone());
                    if request.get("id").is_none() {
                        continue;
                    }
                    let response = json!({"jsonrpc":"2.0", "id":request["id"], "result":result});
                    write
                        .write_all(format!("{response}\n").as_bytes())
                        .await
                        .unwrap();
                    break;
                }
            }
            std::future::pending::<()>().await;
        });
        let (read, write) = tokio::io::split(client);
        ClientConfig::default()
            .with_protocol_version(PROTOCOL)
            .serve(
                AsyncRwTransport::<RoleClient, _, _>::new(read, write)
                    .with_max_line_length(MESSAGE_BYTES_MAX),
            )
            .await
            .unwrap()
    });
    (
        Connection {
            definition,
            runtime,
            service: Some(service),
            child: None,
            tools: vec![tool("echo")],
            resources: Vec::new(),
            prompts: Vec::new(),
        },
        captured,
    )
}
pub(crate) fn initialized() -> Value {
    json!({"protocolVersion":"2025-11-25", "serverInfo":{"name":"fixture","version":"1"},"capabilities":{}})
}
fn tool(name: &str) -> Value {
    json!({"name":name,"inputSchema":{"type":"object"}})
}

#[test]
fn edited_definition_invalidates_connection_authority() {
    let fixture = Fixture::new();
    let definition = fixture.definition();
    fs::write(fixture.0.join("mcp/fixture.md"), "changed").unwrap();
    assert!(definition.validate().is_err());
}
#[test]
fn discovery_collects_two_pages() {
    let fixture = Fixture::new();
    let connection = fixture_connection(
        &fixture,
        vec![
            initialized(),
            json!({"tools":[tool("one")],"nextCursor":"two"}),
            json!({"tools":[tool("two")]}),
        ],
    );
    let tools = connection.list("tools", &AtomicBool::new(false)).unwrap();
    assert_eq!(tools.len(), 2);
}
#[test]
fn duplicate_tool_names_are_rejected_across_pages() {
    let fixture = Fixture::new();
    let connection = fixture_connection(
        &fixture,
        vec![
            initialized(),
            json!({"tools":[tool("same")],"nextCursor":"two"}),
            json!({"tools":[tool("same")]}),
        ],
    );
    assert!(
        connection
            .list("tools", &AtomicBool::new(false))
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}
#[test]
fn repeated_cursor_is_rejected() {
    let fixture = Fixture::new();
    let connection = fixture_connection(
        &fixture,
        vec![
            initialized(),
            json!({"tools":[],"nextCursor":"same"}),
            json!({"tools":[],"nextCursor":"same"}),
        ],
    );
    assert!(connection.list("tools", &AtomicBool::new(false)).is_err());
}
#[test]
fn cancellation_before_request_sends_nothing() {
    let fixture = Fixture::new();
    let connection = fixture_connection(&fixture, vec![initialized()]);
    assert!(
        connection
            .request("ping", json!({}), &AtomicBool::new(true))
            .is_err()
    );
}
#[test]
fn tool_outside_catalog_is_never_sent() {
    let fixture = Fixture::new();
    let connection = fixture_connection(&fixture, vec![initialized()]);
    assert!(
        connection
            .tool(
                "unknown",
                json!({}),
                &AtomicBool::new(false),
                REQUEST_TIME_MAX
            )
            .is_err()
    );
}
#[test]
fn oversized_result_is_refused() {
    assert!(bounded_result(json!({"content":"x".repeat(RESULT_BYTES_MAX)})).is_err());
}
#[test]
fn structured_and_error_results_are_preserved() {
    let result = json!({"content":[], "structuredContent":{"ok":false}, "isError":true});
    assert_eq!(
        serde_json::from_str::<Value>(&bounded_result(result.clone()).unwrap()).unwrap(),
        result
    );
}
#[test]
fn oversized_request_is_refused_before_transport() {
    let fixture = Fixture::new();
    let connection = fixture_connection(&fixture, vec![initialized()]);
    assert!(
        connection
            .request(
                "tools/call",
                json!({"name":"x","arguments":{"text":"x".repeat(8192)}}),
                &AtomicBool::new(false)
            )
            .is_err()
    );
}

#[test]
fn request_timeout_is_bounded_without_a_server_response() {
    let fixture = Fixture::new();
    let connection = fixture_connection(&fixture, vec![initialized()]);
    assert!(
        connection
            .request_with_timeout("ping", json!({}), &AtomicBool::new(false), Duration::ZERO)
            .is_err()
    );
}

#[test]
fn catalog_item_limit_is_enforced() {
    let fixture = Fixture::new();
    let tools = (0..=ITEMS_MAX)
        .map(|i| tool(&format!("tool-{i}")))
        .collect::<Vec<_>>();
    let connection = fixture_connection(&fixture, vec![initialized(), json!({"tools":tools})]);
    assert!(connection.list("tools", &AtomicBool::new(false)).is_err());
}

#[test]
fn tool_result_preserves_server_error_flag() {
    let fixture = Fixture::new();
    let mut connection = fixture_connection(
        &fixture,
        vec![
            initialized(),
            json!({"content":[{"type":"text","text":"failed"}],"isError":true}),
        ],
    );
    connection.tools.push(tool("echo"));
    let text = connection
        .tool("echo", json!({}), &AtomicBool::new(false), REQUEST_TIME_MAX)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap()["isError"],
        true
    );
}

#[cfg(feature = "mcp-http")]
#[test]
fn remote_endpoints_require_encryption_or_literal_loopback() {
    for url in [
        "http://example.org/mcp",
        "http://localhost/mcp",
        "https://user:secret@example.org/mcp",
        "https://example.org/mcp?token=bad",
        "https:///",
        "https://example.org/#secret",
    ] {
        assert!(super::http::validate_url(url).is_err(), "{url}");
    }
    for url in [
        "https://example.org/mcp",
        "http://127.0.0.1:3000/mcp",
        "http://[::1]:3000/mcp",
    ] {
        assert!(super::http::validate_url(url).is_ok(), "{url}");
    }
}
