//! Explicitly trusted MCP connections; server metadata never grants authority.
#[cfg(feature = "mcp-http")]
mod http;
#[cfg(feature = "mcp-oauth")]
mod oauth;
#[cfg(test)]
pub(crate) mod tests;
mod transport;

use crate::workspace::{Mcp, Transport, Workspace};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use rmcp::{
    RoleClient, ServiceExt,
    model::{ClientConfig, ClientRequest, Implementation, ProtocolVersion},
    service::{PeerRequestOptions, RunningService},
    transport::async_rw::AsyncRwTransport,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, runtime::Runtime};

pub const MESSAGE_BYTES_MAX: usize = 64 * 1024;
pub const RESULT_BYTES_MAX: usize = 16 * 1024;
const CONNECTION_BYTES_MAX: u64 = 4 * 1024 * 1024;
const CATALOG_BYTES_MAX: usize = 64 * 1024;
const ITEMS_MAX: usize = 128;
const PAGES_MAX: usize = 16;
const REQUEST_TIME_MAX: Duration = Duration::from_secs(30);
const PROTOCOL: ProtocolVersion = ProtocolVersion::V_2025_11_25;

#[derive(clap::Args)]
pub struct Options {
    id: String,
    #[command(subcommand)]
    action: Operation,
}
#[derive(Debug, clap::Subcommand)]
enum Operation {
    Inspect,
    Read {
        uri: String,
    },
    Prompt {
        name: String,
        #[arg(long, default_value = "{}")]
        arguments: String,
    },
}

pub fn run(root: &Path, options: Options) -> io::Result<()> {
    use std::io::{BufRead, Read, Write};
    let definition = Definition::load(root, &options.id)?;
    let mut output = crate::term::Output::default();
    let mut stderr = io::stderr().lock();
    output.write(
        &format!(
            "{}\nrequested operation: {:?}\n",
            definition.preview(),
            options.action
        ),
        &mut stderr,
    )?;
    stderr.flush()?;
    let mut line = String::new();
    io::stdin().lock().take(16).read_line(&mut line)?;
    if line.trim_end_matches(['\r', '\n']) != "y" {
        return Err(fail("mcp connection denied; no server started"));
    }
    let cancelled = AtomicBool::new(false);
    let connection = Connection::connect(definition, &cancelled, |url| {
        output.write(url, &mut stderr)?;
        stderr.flush()
    })?;
    let result = match options.action {
        Operation::Inspect => {
            json!({"server":connection.id(),"destination":connection.destination(),"protocol":"2025-11-25","tools":connection.tools,"resources":connection.resources,"prompts":connection.prompts})
        }
        Operation::Read { uri } => {
            if !connection.resources.iter().any(|item| item["uri"] == uri) {
                return Err(fail("resource is outside the discovered catalog"));
            }
            connection.request("resources/read", json!({"uri":uri}), &cancelled)?
        }
        Operation::Prompt { name, arguments } => {
            if !connection.prompts.iter().any(|item| item["name"] == name) {
                return Err(fail("prompt is outside the discovered catalog"));
            }
            if arguments.len() > 8192 {
                return Err(fail("prompt arguments exceed 8192 bytes"));
            }
            let arguments: std::collections::BTreeMap<String, String> =
                serde_json::from_str(&arguments).map_err(io::Error::other)?;
            connection.request(
                "prompts/get",
                json!({"name":name,"arguments":arguments}),
                &cancelled,
            )?
        }
    };
    let text = bounded_result(result)?;
    output.write(&format!("{text}\n"), &mut io::stdout().lock())
}

pub struct Definition {
    config: Mcp,
    root: PathBuf,
    source: String,
    #[cfg(feature = "mcp-http")]
    access_token: Option<String>,
}

impl Definition {
    pub fn load(root: &Path, id: &str) -> io::Result<Self> {
        let (config, source) = Workspace::open(root)?.mcp(id)?;
        if let Transport::Stdio { command, .. } = &config.transport {
            let path = Path::new(command);
            if !path.is_absolute() || !path.is_file() || path.canonicalize()? != path {
                return Err(fail(
                    "mcp executable must be an absolute canonical file path",
                ));
            }
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd")
                })
            {
                return Err(fail(
                    "mcp batch scripts are unsupported; select an executable",
                ));
            }
        }
        #[cfg(feature = "mcp-http")]
        let mut config = config;
        #[cfg(feature = "mcp-http")]
        if let Transport::Http { url, .. } = &mut config.transport {
            http::validate_url(url)?;
            *url = reqwest::Url::parse(url)
                .map_err(|_| fail("invalid mcp endpoint"))?
                .to_string();
        }
        Ok(Self {
            config,
            root: root.canonicalize()?,
            source,
            #[cfg(feature = "mcp-http")]
            access_token: None,
        })
    }

    pub fn validate(&self) -> io::Result<()> {
        let (_, current) = Workspace::open(&self.root)?.mcp(&self.config.id)?;
        if self.source != current {
            return Err(fail(
                "mcp definition changed; disconnect and approve it again",
            ));
        }
        Ok(())
    }

    #[cfg(feature = "gguf")]
    pub fn credential_values(&self) -> io::Result<Vec<String>> {
        let references = match &self.config.transport {
            Transport::Stdio { env, .. } => env.values().collect::<Vec<_>>(),
            Transport::Http { token_env, .. } => token_env.iter().collect(),
        };
        references
            .into_iter()
            .map(|name| {
                let value = std::env::var(name)
                    .map_err(|_| fail("mcp credential reference is unset or not Unicode"))?;
                if value.len() > 4096 {
                    return Err(fail("mcp credential exceeds 4096 bytes"));
                }
                Ok(value)
            })
            .collect()
    }

    pub fn preview(&self) -> String {
        let destination = match &self.config.transport {
            Transport::Stdio { command, args, env } => format!(
                "executable: {command:?}\nworking directory: {:?}\narguments: {args:?}\nenvironment references: {env:?}\nruns with your OS privileges; may access files or network; not sandboxed",
                self.root
            ),
            Transport::Http {
                url,
                token_env,
                oauth,
            } => format!(
                "network destination: {url:?}\ncredential reference: {token_env:?}\nOAuth identity: {oauth:?}\npermits external traffic to this endpoint"
            ),
        };
        format!(
            "connect server: {}\n{destination}\nserver metadata and results remain untrusted\n[y] approve once · [n/Esc/Enter] deny",
            self.config.id
        )
    }
}

pub struct Connection {
    definition: Definition,
    runtime: Runtime,
    service: Option<RunningService<RoleClient, ClientConfig>>,
    child: Option<Box<dyn ChildWrapper>>,
    tools: Vec<Value>,
    resources: Vec<Value>,
    prompts: Vec<Value>,
}

impl Connection {
    /// Starts only after the caller has approved the exact definition preview.
    pub fn connect(
        definition: Definition,
        cancelled: &AtomicBool,
        mut show: impl FnMut(&str) -> io::Result<()>,
    ) -> io::Result<Self> {
        definition.validate()?;
        check(cancelled, Instant::now())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut connection = Self {
            definition,
            runtime,
            service: None,
            child: None,
            tools: Vec::new(),
            resources: Vec::new(),
            prompts: Vec::new(),
        };
        #[cfg(feature = "mcp-oauth")]
        if let Transport::Http {
            oauth: Some(config),
            url,
            ..
        } = &connection.definition.config.transport
        {
            let token = oauth::authorize(&connection.runtime, url, config, cancelled, &mut show)?;
            connection.definition.access_token = Some(token);
        }
        #[cfg(not(feature = "mcp-oauth"))]
        {
            let _ = &mut show;
            if matches!(
                connection.definition.config.transport,
                Transport::Http { oauth: Some(_), .. }
            ) {
                return Err(fail("OAuth requires the optional mcp-oauth feature"));
            }
        }
        connection.open(cancelled)?;
        let info = connection
            .service
            .as_ref()
            .and_then(|s| s.peer_info())
            .ok_or_else(|| fail("mcp initialization returned no server information"))?;
        if info.protocol_version != PROTOCOL {
            return Err(fail(
                "mcp server selected an unsupported protocol; expected 2025-11-25",
            ));
        }
        let capabilities = serde_json::to_value(&info.capabilities).map_err(io::Error::other)?;
        if capabilities.get("tools").is_some() {
            connection.tools = connection.list("tools", cancelled)?;
        }
        if capabilities.get("resources").is_some() {
            connection.resources = connection.list("resources", cancelled)?;
        }
        if capabilities.get("prompts").is_some() {
            connection.prompts = connection.list("prompts", cancelled)?;
        }
        Ok(connection)
    }

    fn open(&mut self, cancelled: &AtomicBool) -> io::Result<()> {
        let _entered = self.runtime.enter();
        #[cfg(feature = "mcp-http")]
        if let Transport::Http { url, token_env, .. } = &self.definition.config.transport {
            use rmcp::transport::{
                StreamableHttpClientTransport, common::client_side_sse::NeverRetry,
                streamable_http_client::StreamableHttpClientTransportConfig,
            };
            let mut config = StreamableHttpClientTransportConfig::with_uri(url.clone());
            config.max_concurrent_requests = 1;
            config.channel_buffer_capacity = 4;
            config.max_sse_event_size = MESSAGE_BYTES_MAX;
            config.reinit_on_expired_session = false;
            config.retry_config = std::sync::Arc::new(NeverRetry::default());
            config.auth_header.clone_from(&self.definition.access_token);
            if let Some(reference) = token_env {
                let token = std::env::var(reference)
                    .map_err(|_| fail("mcp token environment reference is unset or not Unicode"))?;
                if token.is_empty()
                    || token.len() > 4096
                    || !token.bytes().all(|byte| byte.is_ascii_graphic())
                {
                    return Err(fail("mcp token must contain 1..4096 visible ASCII bytes"));
                }
                config.auth_header = Some(token);
            }
            let transport =
                StreamableHttpClientTransport::with_client(http::Client::new()?, config);
            let info = ClientConfig::new(
                Default::default(),
                Implementation::new("sefai", env!("CARGO_PKG_VERSION")),
            )
            .with_protocol_version(PROTOCOL);
            self.service = Some(self.runtime.block_on(async {
                tokio::select! {
                    result = tokio::time::timeout(REQUEST_TIME_MAX, info.serve(transport::Bounded::new(transport))) => result.map_err(|_| fail("mcp initialization timed out"))?.map_err(|_| fail("mcp HTTP initialization failed; check endpoint and resource-specific credential")),
                    _ = cancellation(cancelled) => Err(fail("mcp connection cancelled")),
                }
            })?);
            return Ok(());
        }
        let Transport::Stdio { command, args, env } = &self.definition.config.transport else {
            return Err(fail("remote mcp transport is not enabled in this build"));
        };
        let mut command = tokio::process::Command::new(command);
        command
            .args(args)
            .env_clear()
            .current_dir(&self.definition.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        for (name, reference) in env {
            let value = std::env::var_os(reference)
                .ok_or_else(|| fail("mcp environment reference is unset"))?;
            if value.len() > 4096 {
                return Err(fail("mcp environment value exceeds 4096 bytes"));
            }
            command.env(name, value);
        }
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(KillOnDrop);
        #[cfg(unix)]
        wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(process_wrap::tokio::JobObject);
        let mut child = wrapped.spawn()?;
        let stdout = child
            .inner_mut()
            .stdout()
            .take()
            .ok_or_else(|| fail("mcp stdout unavailable"))?;
        let stdin = child
            .inner_mut()
            .stdin()
            .take()
            .ok_or_else(|| fail("mcp stdin unavailable"))?;
        self.child = Some(child);
        let transport =
            AsyncRwTransport::<RoleClient, _, _>::new(stdout.take(CONNECTION_BYTES_MAX), stdin)
                .with_max_line_length(MESSAGE_BYTES_MAX);
        let info = ClientConfig::new(
            Default::default(),
            Implementation::new("sefai", env!("CARGO_PKG_VERSION")),
        )
        .with_protocol_version(PROTOCOL);
        self.service = Some(self.runtime.block_on(async {
            tokio::select! {
                result = tokio::time::timeout(REQUEST_TIME_MAX, info.serve(transport::Bounded::new(transport))) => {
                    result.map_err(|_| fail("mcp initialization timed out"))?.map_err(|_| fail("mcp initialization failed"))
                }
                _ = cancellation(cancelled) => Err(fail("mcp connection cancelled")),
            }
        })?);
        Ok(())
    }

    #[cfg(feature = "gguf")]
    pub fn credential_values(&self) -> Vec<String> {
        #[cfg(feature = "mcp-http")]
        {
            self.definition.access_token.iter().cloned().collect()
        }
        #[cfg(not(feature = "mcp-http"))]
        {
            Vec::new()
        }
    }

    #[cfg(feature = "gguf")]
    pub fn tools(&self) -> &[Value] {
        &self.tools
    }

    #[cfg(any(feature = "gguf", test))]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool["name"] == name)
    }

    pub fn id(&self) -> &str {
        &self.definition.config.id
    }

    pub fn destination(&self) -> String {
        match &self.definition.config.transport {
            Transport::Stdio { command, .. } => {
                format!("local process {command:?}; may access network")
            }
            Transport::Http { url, .. } => url.clone(),
        }
    }

    pub fn validate(&self) -> io::Result<()> {
        self.definition.validate()
    }

    pub fn request(
        &self,
        method: &str,
        params: Value,
        cancelled: &AtomicBool,
    ) -> io::Result<Value> {
        self.request_with_timeout(method, params, cancelled, REQUEST_TIME_MAX)
    }

    fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        cancelled: &AtomicBool,
        timeout: Duration,
    ) -> io::Result<Value> {
        self.validate()?;
        check(cancelled, Instant::now())?;
        if serde_json::to_vec(&params).map_err(io::Error::other)?.len() > 8192 {
            return Err(fail("mcp request arguments exceed 8192 bytes"));
        }
        let request: ClientRequest =
            serde_json::from_value(json!({"method":method,"params":params}))
                .map_err(io::Error::other)?;
        let service = self
            .service
            .as_ref()
            .ok_or_else(|| fail("mcp is disconnected"))?;
        let timeout = timeout.min(REQUEST_TIME_MAX);
        let result = self.runtime.block_on(async {
            let operation = async {
                service.send_request_with_option(request, PeerRequestOptions::with_timeout(timeout))
                    .await?.await_response().await
            };
            tokio::select! {
                result = tokio::time::timeout(timeout, operation) => {
                    result.map_err(|_| fail("mcp request timed out; outcome may be unknown"))?
                        .map_err(|_| fail("mcp request failed; outcome may be unknown"))
                }
                _ = cancellation(cancelled) => Err(fail("mcp request cancelled; outcome may be unknown")),
            }
        })?;
        let value = serde_json::to_value(result).map_err(io::Error::other)?;
        if serde_json::to_vec(&value).map_err(io::Error::other)?.len() > MESSAGE_BYTES_MAX {
            return Err(fail("mcp response exceeds 65536 bytes"));
        }
        Ok(value)
    }

    fn list(&self, kind: &str, cancelled: &AtomicBool) -> io::Result<Vec<Value>> {
        let mut result = Vec::new();
        let mut cursor = None;
        let mut seen = BTreeSet::new();
        let mut bytes = 0;
        for _ in 0..PAGES_MAX {
            let page =
                self.request(&format!("{kind}/list"), json!({"cursor":cursor}), cancelled)?;
            let items = page[kind]
                .as_array()
                .ok_or_else(|| fail("mcp discovery response has no item list"))?;
            for item in items {
                let key = item[if kind == "resources" { "uri" } else { "name" }]
                    .as_str()
                    .ok_or_else(|| fail("mcp discovery item has no identity"))?;
                if key.is_empty()
                    || key.len() > 256
                    || !seen.insert(format!("item:{key}"))
                    || (kind != "resources"
                        && !key
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-./:".contains(&b)))
                {
                    return Err(fail(
                        "mcp discovery returned an invalid or duplicate identity",
                    ));
                }
                bytes += serde_json::to_vec(item).map_err(io::Error::other)?.len();
                if result.len() == ITEMS_MAX || bytes > CATALOG_BYTES_MAX {
                    return Err(fail("mcp catalog exceeds 128 items or 65536 bytes"));
                }
                result.push(item.clone());
            }
            cursor = page["nextCursor"].as_str().map(str::to_owned);
            let Some(next) = &cursor else {
                return Ok(result);
            };
            if next.len() > 1024 || !seen.insert(format!("cursor:{next}")) {
                return Err(fail("mcp discovery cursor is oversized or repeated"));
            }
        }
        Err(fail("mcp discovery exceeds 16 pages"))
    }

    #[cfg(any(feature = "gguf", test))]
    pub fn tool(
        &self,
        name: &str,
        arguments: Value,
        cancelled: &AtomicBool,
        timeout: Duration,
    ) -> io::Result<String> {
        if !self.has_tool(name) {
            return Err(fail("mcp tool is outside the frozen catalog"));
        }
        let value = self.request_with_timeout(
            "tools/call",
            json!({"name": name, "arguments": arguments}),
            cancelled,
            timeout,
        )?;
        bounded_result(value)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.runtime.block_on(async {
            if let Some(service) = self.service.take() {
                let _ = tokio::time::timeout(Duration::from_secs(2), service.cancel()).await;
            }
            if let Some(mut child) = self.child.take() {
                let _ =
                    tokio::time::timeout(Duration::from_secs(3), Box::into_pin(child.kill())).await;
            }
        });
    }
}

fn bounded_result(value: Value) -> io::Result<String> {
    let text = serde_json::to_string(&value).map_err(io::Error::other)?;
    if text.len() > RESULT_BYTES_MAX {
        return Err(fail("mcp result exceeds 16384 bytes"));
    }
    Ok(text)
}

async fn cancellation(cancelled: &AtomicBool) {
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn check(cancelled: &AtomicBool, started: Instant) -> io::Result<()> {
    if cancelled.load(Ordering::Relaxed) || started.elapsed() >= REQUEST_TIME_MAX {
        return Err(fail("mcp operation cancelled or timed out"));
    }
    Ok(())
}

fn fail(message: &str) -> io::Error {
    io::Error::other(message)
}
