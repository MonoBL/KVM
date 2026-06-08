use crate::protocol::Message;
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Send a length-prefixed (u32 LE) bincode message over any async writer.
pub async fn send_msg<W: AsyncWriteExt + Unpin>(writer: &mut W, msg: &Message) -> Result<()> {
    let payload = bincode::serialize(msg)?;
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&payload).await?;
    Ok(())
}

/// Receive a length-prefixed bincode message from any async reader.
pub async fn recv_msg<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Message> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(bincode::deserialize(&buf)?)
}

/// Convenience: split a TcpStream so send and recv can be used concurrently.
pub fn split(
    stream: TcpStream,
) -> (
    tokio::net::tcp::OwnedWriteHalf,
    tokio::net::tcp::OwnedReadHalf,
) {
    let (r, w) = stream.into_split();
    (w, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::VERSION;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn loopback_hello_heartbeat() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            let msg = recv_msg(&mut conn).await.unwrap();
            assert!(matches!(msg, Message::Hello { .. }));
            send_msg(&mut conn, &Message::Welcome { peer_id: 1 }).await.unwrap();
            let hb = recv_msg(&mut conn).await.unwrap();
            assert_eq!(hb, Message::Heartbeat);
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        send_msg(
            &mut client,
            &Message::Hello {
                name: "loopback-client".into(),
                screen_w: 1920,
                screen_h: 1080,
                version: VERSION,
            },
        )
        .await
        .unwrap();
        let welcome = recv_msg(&mut client).await.unwrap();
        assert!(matches!(welcome, Message::Welcome { peer_id: 1 }));
        send_msg(&mut client, &Message::Heartbeat).await.unwrap();

        server.await.unwrap();
    }

    #[tokio::test]
    async fn split_halves_work() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let (mut w, mut r) = split(tcp);
            let msg = recv_msg(&mut r).await.unwrap();
            assert_eq!(msg, Message::Heartbeat);
            send_msg(&mut w, &Message::Heartbeat).await.unwrap();
        });

        let tcp = TcpStream::connect(addr).await.unwrap();
        let (mut w, mut r) = split(tcp);
        send_msg(&mut w, &Message::Heartbeat).await.unwrap();
        let reply = recv_msg(&mut r).await.unwrap();
        assert_eq!(reply, Message::Heartbeat);
        server.await.unwrap();
    }
}
