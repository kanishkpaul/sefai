//! Streamable HTTP with bounded bodies, explicit credentials and no redirects or replay.
use super::{MESSAGE_BYTES_MAX, REQUEST_TIME_MAX, fail};
use futures_util::{StreamExt, stream::BoxStream};
use http::{HeaderName, HeaderValue};
use rmcp::{
    model::ClientJsonRpcMessage,
    transport::streamable_http_client::{
        SseError, StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
    },
};
use sse_stream::{Sse, SseStream};
use std::{collections::HashMap, io, sync::Arc};

type Error = StreamableHttpError<io::Error>;

#[derive(Clone)]
pub struct Client(reqwest::Client);
impl Client {
    pub fn new() -> io::Result<Self> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(REQUEST_TIME_MAX)
            .pool_max_idle_per_host(0)
            .build()
            .map(Self)
            .map_err(|_| fail("creating mcp HTTPS client failed"))
    }

    fn request(
        &self,
        method: reqwest::Method,
        uri: &str,
        session: Option<&str>,
        auth: Option<&str>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> reqwest::RequestBuilder {
        let mut request = self
            .0
            .request(method, uri)
            .header("accept", "application/json, text/event-stream");
        if let Some(session) = session {
            request = request.header("mcp-session-id", session);
        }
        if let Some(auth) = auth {
            request = request.bearer_auth(auth);
        }
        request.headers(headers.into_iter().collect())
    }
}

impl StreamableHttpClient for Client {
    type Error = io::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, Error> {
        let response = self
            .request(
                reqwest::Method::POST,
                &uri,
                session.as_deref(),
                auth.as_deref(),
                headers,
            )
            .json(&message)
            .send()
            .await
            .map_err(|_| Error::Client(fail("mcp HTTP request failed")))?;
        let status = response.status();
        if status == reqwest::StatusCode::ACCEPTED || status == reqwest::StatusCode::NO_CONTENT {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if !status.is_success() {
            // Never copy server error bodies or credential challenges into durable logs.
            return Err(Error::Client(fail(&format!(
                "mcp HTTP status {}; authorization may require a resource-specific OAuth token",
                status.as_u16()
            ))));
        }
        let session = response
            .headers()
            .get("mcp-session-id")
            .map(|header| {
                let value = header
                    .to_str()
                    .map_err(|_| Error::Client(fail("mcp session identifier is invalid")))?;
                if value.len() > 256 {
                    return Err(Error::Client(fail(
                        "mcp session identifier exceeds 256 bytes",
                    )));
                }
                Ok(value.to_owned())
            })
            .transpose()?;
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_owned();
        if content_type == "text/event-stream" {
            return Ok(StreamableHttpPostResponse::Sse(sse(response), session));
        }
        if content_type != "application/json" {
            return Err(Error::Client(fail(
                "mcp HTTP response has unsupported content type",
            )));
        }
        let body = body(response).await.map_err(Error::Client)?;
        let parsed = serde_json::from_slice(&body).map_err(|_| {
            Error::Client(fail("mcp HTTP response is not a valid protocol message"))
        })?;
        Ok(StreamableHttpPostResponse::Json(parsed, session))
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), Error> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &uri,
                Some(&session),
                auth.as_deref(),
                headers,
            )
            .send()
            .await
            .map_err(|_| Error::Client(fail("mcp session cleanup failed")))?;
        if response.status().is_success()
            || response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
        {
            return Ok(());
        }
        Err(Error::Client(fail("mcp session cleanup was rejected")))
    }

    async fn get_stream(
        &self,
        _uri: Arc<str>,
        _session: Option<Arc<str>>,
        _last_event: Option<String>,
        _auth: Option<String>,
        _headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, Error> {
        // Standalone notifications and resumptions are optional; never reconnect a failed write.
        Err(StreamableHttpError::ServerDoesNotSupportSse)
    }
}

async fn body(mut response: reqwest::Response) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| fail("reading mcp HTTP body failed"))?
    {
        if bytes.len() + chunk.len() > MESSAGE_BYTES_MAX {
            return Err(fail("mcp HTTP body exceeds 65536 bytes"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn sse(response: reqwest::Response) -> BoxStream<'static, Result<Sse, SseError>> {
    let bytes =
        futures_util::stream::try_unfold((response, 0), |(mut response, count)| async move {
            match response
                .chunk()
                .await
                .map_err(|_| fail("reading mcp SSE body failed"))?
            {
                None => Ok(None),
                Some(chunk) if count + chunk.len() <= MESSAGE_BYTES_MAX => {
                    let count = count + chunk.len();
                    Ok(Some((chunk, (response, count))))
                }
                Some(_) => Err(fail("mcp SSE body exceeds 65536 bytes")),
            }
        });
    SseStream::from_bytes_stream(bytes).boxed()
}

pub fn validate_url(url: &str) -> io::Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| fail("mcp endpoint is not a valid URL"))?;
    let loopback = parsed
        .host_str()
        .is_some_and(|host| host == "127.0.0.1" || host == "[::1]");
    if (parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback))
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(fail(
            "mcp endpoint requires HTTPS or literal loopback HTTP without credentials, query or fragment",
        ));
    }
    Ok(())
}
