//! Optional authorization-code login with SDK PKCE/state checks and pinned HTTP origins.
use super::{MESSAGE_BYTES_MAX, REQUEST_TIME_MAX, cancellation, fail, http::validate_url};
use crate::workspace::OAuth;
use rmcp::transport::auth::{
    AuthorizationRequest, OAuthHttpClient, OAuthHttpClientFuture, OAuthHttpRequest, OAuthState,
};
use std::{
    io,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    runtime::Runtime,
};

struct Client {
    http: reqwest::Client,
    resource_origin: String,
    issuer_origin: String,
    issuer: String,
}
impl OAuthHttpClient for Client {
    fn execute(&self, operation: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        Box::pin(async move {
            let url = reqwest::Url::parse(&operation.request.uri().to_string())?;
            validate_url_base(&url)?;
            let origin = url.origin().ascii_serialization();
            if origin != self.resource_origin && origin != self.issuer_origin {
                return Err(fail("OAuth metadata attempted an unapproved origin").into());
            }
            if operation.request.body().len() > 8192 {
                return Err(fail("OAuth request exceeds 8192 bytes").into());
            }
            let request = reqwest::Request::try_from(operation.request)?;
            let mut response = self.http.execute(request).await?;
            let mut builder = http::Response::builder()
                .status(response.status())
                .version(response.version());
            for (key, value) in response.headers() {
                builder = builder.header(key, value);
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if body.len() + chunk.len() > MESSAGE_BYTES_MAX {
                    return Err(fail("OAuth response exceeds 65536 bytes").into());
                }
                body.extend_from_slice(&chunk);
            }
            if let Ok(metadata) = serde_json::from_slice::<serde_json::Value>(&body)
                && let Some(issuer) = metadata["issuer"].as_str()
                && issuer != self.issuer
            {
                return Err(fail("OAuth metadata issuer differs from the approved issuer").into());
            }
            Ok(builder.body(body)?)
        })
    }
}

pub fn authorize(
    runtime: &Runtime,
    resource: &str,
    config: &OAuth,
    cancelled: &AtomicBool,
    show: &mut impl FnMut(&str) -> io::Result<()>,
) -> io::Result<String> {
    validate_url(&config.issuer)?;
    let resource = reqwest::Url::parse(resource).map_err(|_| fail("invalid OAuth resource URL"))?;
    let issuer =
        reqwest::Url::parse(&config.issuer).map_err(|_| fail("invalid OAuth issuer URL"))?;
    let callback = reqwest::Url::parse(&config.redirect_uri)
        .map_err(|_| fail("invalid OAuth redirect URI"))?;
    if callback.scheme() != "http"
        || callback.host_str() != Some("127.0.0.1")
        || !callback.username().is_empty()
        || callback.password().is_some()
        || callback.query().is_some()
        || callback.fragment().is_some()
        || callback.path() != "/callback"
        || callback.port().is_none_or(|port| port == 0)
    {
        return Err(fail(
            "OAuth redirect must be http://127.0.0.1:PORT/callback with a nonzero explicit port",
        ));
    }
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIME_MAX)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| fail("creating OAuth HTTPS client failed"))?;
    let client = Arc::new(Client {
        http,
        resource_origin: resource.origin().ascii_serialization(),
        issuer_origin: issuer.origin().ascii_serialization(),
        issuer: config.issuer.clone(),
    });
    runtime.block_on(async {
        let operation = async {
            // Bind before opening authorization so another process cannot claim the callback port.
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, callback.port().expect("validated explicit callback port"))).await?;
            let mut state = OAuthState::new_with_oauth_http_client(resource.as_str(), client).await.map_err(|_| fail("creating OAuth state failed"))?;
            let request = AuthorizationRequest::new(&config.redirect_uri)
                .with_preregistered_client(&config.client_id).with_scopes(config.scopes.clone());
            state.start_authorization(request).await.map_err(|_| fail("OAuth discovery or client setup failed"))?;
            let authorization = state.get_authorization_url().await.map_err(|_| fail("OAuth authorization URL unavailable"))?;
            let parsed = reqwest::Url::parse(&authorization).map_err(|_| fail("invalid OAuth authorization URL"))?;
            if parsed.origin() != issuer.origin() || parsed.scheme() != issuer.scheme() {
                return Err(fail("OAuth authorization URL does not match the approved issuer origin"));
            }
            show(&format!("Open this URL in your browser to authorize the selected MCP server:\n{authorization}\nWaiting up to 120s for the loopback callback; Ctrl+C cancels. Credentials stay in this process.\n"))?;
            let (mut socket, _) = listener.accept().await?;
            let mut bytes = Vec::new();
            let mut buffer = [0; 1024];
            while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).await?;
                if count == 0 || bytes.len() + count > 8192 { return Err(fail("OAuth callback is truncated or exceeds 8192 bytes")); }
                bytes.extend_from_slice(&buffer[..count]);
            }
            let line = std::str::from_utf8(&bytes).map_err(|_| fail("OAuth callback is not UTF-8"))?.lines().next().ok_or_else(|| fail("OAuth callback has no request line"))?;
            let path = line.strip_prefix("GET ").and_then(|s| s.strip_suffix(" HTTP/1.1")).ok_or_else(|| fail("OAuth callback requires an HTTP/1.1 GET"))?;
            let returned = callback.join(path).map_err(|_| fail("invalid OAuth callback URL"))?;
            if !path.starts_with("/callback?") || returned.origin() != callback.origin() || returned.path() != callback.path() || returned.fragment().is_some() {
                return Err(fail("OAuth callback destination changed"));
            }
            state.handle_callback_url(returned.as_str()).await.map_err(|_| fail("OAuth callback state, issuer or code verification failed"))?;
            let (_, tokens) = state.get_credentials().await.map_err(|_| fail("OAuth credentials unavailable"))?;
            let tokens = serde_json::to_value(tokens).map_err(io::Error::other)?;
            let access = tokens["access_token"].as_str().filter(|token| !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| b.is_ascii_graphic())).ok_or_else(|| fail("OAuth access token is missing or exceeds limits"))?.to_owned();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 25\r\nConnection: close\r\n\r\nReturn to sefai to proceed").await?;
            Ok(access)
        };
        tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(120), operation) => result.map_err(|_| fail("OAuth authorization timed out"))?,
            _ = cancellation(cancelled) => Err(fail("OAuth authorization cancelled")),
        }
    })
}

fn validate_url_base(url: &reqwest::Url) -> io::Result<()> {
    let mut base = url.clone();
    base.set_query(None);
    validate_url(base.as_str())
}
