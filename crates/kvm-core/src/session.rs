use crate::protocol::{Message, VERSION};
use crate::transport::{recv_msg, send_msg};
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::TcpStream;

static NEXT_PEER_ID: AtomicU64 = AtomicU64::new(1);

fn alloc_peer_id() -> u64 {
    NEXT_PEER_ID.fetch_add(1, Ordering::Relaxed)
}

/// A fully-handshaked TCP connection to one peer.
pub struct PeerSession {
    pub stream: TcpStream,
    /// ID assigned by the server; matches the Screen.id in EngineState.
    pub peer_id: u64,
    pub peer_name: String,
    /// Remote screen dimensions reported in Hello.
    pub screen_w: u32,
    pub screen_h: u32,
}

/// Server-side handshake: receive Hello, send Welcome.
///
/// Returns a `PeerSession` that owns the TCP stream.
pub async fn server_handshake(mut stream: TcpStream, _local_name: &str) -> Result<PeerSession> {
    let msg = recv_msg(&mut stream).await?;
    match msg {
        Message::Hello { name, screen_w, screen_h, version } => {
            if version != VERSION {
                anyhow::bail!("protocol version mismatch: peer={} local={}", version, VERSION);
            }
            let peer_id = alloc_peer_id();
            send_msg(&mut stream, &Message::Welcome { peer_id }).await?;
            Ok(PeerSession { stream, peer_id, peer_name: name, screen_w, screen_h })
        }
        other => anyhow::bail!("expected Hello, got {:?}", other),
    }
}

/// Client-side handshake: connect, send Hello, receive Welcome.
pub async fn client_handshake(
    host: &str,
    port: u16,
    local_name: &str,
    screen_w: u32,
    screen_h: u32,
) -> Result<PeerSession> {
    let mut stream = TcpStream::connect((host, port)).await?;
    send_msg(
        &mut stream,
        &Message::Hello {
            name: local_name.to_string(),
            screen_w,
            screen_h,
            version: VERSION,
        },
    )
    .await?;
    let reply = recv_msg(&mut stream).await?;
    match reply {
        Message::Welcome { peer_id } => Ok(PeerSession {
            stream,
            peer_id,
            peer_name: host.to_string(),
            screen_w,
            screen_h,
        }),
        other => anyhow::bail!("expected Welcome, got {:?}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn server_client_handshake_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            server_handshake(stream, "server").await.unwrap()
        });

        let client = client_handshake("127.0.0.1", addr.port(), "test-client", 2560, 1440)
            .await
            .unwrap();

        let server = server_task.await.unwrap();

        // Server and client agree on the same peer_id.
        assert_eq!(client.peer_id, server.peer_id);
        assert_eq!(server.peer_name, "test-client");
        assert_eq!(server.screen_w, 2560);
        assert_eq!(server.screen_h, 1440);
    }

    #[tokio::test]
    async fn version_mismatch_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            // server_handshake should error because peer sends wrong version
            let result = server_handshake(stream, "server").await;
            assert!(result.is_err(), "expected version mismatch error");
        });

        // Act as a bad client: send Hello with wrong version directly.
        let mut stream = TcpStream::connect(addr).await.unwrap();
        send_msg(
            &mut stream,
            &Message::Hello {
                name: "bad-peer".into(),
                screen_w: 1920,
                screen_h: 1080,
                version: 9999,
            },
        )
        .await
        .unwrap();

        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn multiple_peers_get_unique_ids() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let mut ids = vec![];
            for _ in 0..3 {
                let (stream, _) = listener.accept().await.unwrap();
                let s = server_handshake(stream, "server").await.unwrap();
                ids.push(s.peer_id);
            }
            ids
        });

        let mut client_ids = vec![];
        for _ in 0..3 {
            let c = client_handshake("127.0.0.1", addr.port(), "c", 1920, 1080)
                .await
                .unwrap();
            client_ids.push(c.peer_id);
        }

        let server_ids = server_task.await.unwrap();
        // All three get distinct IDs.
        assert_eq!(server_ids.len(), 3);
        let mut sorted = server_ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "IDs not unique: {:?}", server_ids);
        // Client and server agree.
        assert_eq!(client_ids, server_ids);
    }
}
