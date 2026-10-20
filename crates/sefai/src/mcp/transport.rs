//! Caps the number of SDK-dispatched messages over one explicitly approved connection.
use super::{MESSAGE_BYTES_MAX, fail};
use rmcp::{
    RoleClient,
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::Transport,
};
use std::io;

const MESSAGES_MAX: usize = 512;
pub struct Bounded<T> {
    inner: T,
    remaining: usize,
}
impl<T> Bounded<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            remaining: MESSAGES_MAX,
        }
    }
}
impl<T: Transport<RoleClient>> Transport<RoleClient> for Bounded<T> {
    type Error = io::Error;
    fn send(
        &mut self,
        message: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = io::Result<()>> + Send + 'static {
        let allowed =
            serde_json::to_vec(&message).is_ok_and(|text| text.len() <= MESSAGE_BYTES_MAX);
        let future = allowed.then(|| self.inner.send(message));
        async move {
            future
                .ok_or_else(|| fail("mcp outgoing message exceeds 65536 bytes"))?
                .await
                .map_err(|_| fail("sending mcp message failed"))
        }
    }
    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        if self.remaining == 0 {
            return None;
        }
        let message = self.inner.receive().await?;
        self.remaining -= 1;
        Some(message)
    }
    async fn close(&mut self) -> io::Result<()> {
        self.inner
            .close()
            .await
            .map_err(|_| fail("closing mcp transport failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::transport::async_rw::AsyncRwTransport;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn exhausted_message_budget_stops_before_the_next_sdk_dispatch() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (client, mut server) = tokio::io::duplex(1024);
            server.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\"}\n").await.unwrap();
            let (read, write) = tokio::io::split(client);
            let mut transport = Bounded { inner: AsyncRwTransport::<RoleClient, _, _>::new(read, write), remaining: 1 };
            assert!(transport.receive().await.is_some());
            assert!(transport.receive().await.is_none());
        });
    }

    #[test]
    fn oversized_outgoing_message_is_refused() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (client, _server) = tokio::io::duplex(1024);
            let (read, write) = tokio::io::split(client);
            let mut transport = Bounded::new(AsyncRwTransport::<RoleClient, _, _>::new(read, write));
            let message = serde_json::from_value(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"echo","arguments":{"text":"x".repeat(MESSAGE_BYTES_MAX)}}})).unwrap();
            assert!(transport.send(message).await.is_err());
        });
    }
}
