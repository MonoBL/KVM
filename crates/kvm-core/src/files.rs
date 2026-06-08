use crate::protocol::Message;
use crate::transport::{recv_msg, send_msg};
use anyhow::Result;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CHUNK_SIZE: usize = 65_536; // 64 KiB

/// Send a file over an established connection.
/// Sends FileOffer -> N * FileChunk -> FileEnd.
pub async fn send_file(stream: &mut TcpStream, id: u64, path: &Path) -> Result<()> {
    let mut file = File::open(path).await?;
    let size = file.metadata().await?.len();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    send_msg(stream, &Message::FileOffer { id, name, size }).await?;

    let mut seq = 0u32;
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        send_msg(
            stream,
            &Message::FileChunk {
                id,
                seq,
                bytes: buf[..n].to_vec(),
            },
        )
        .await?;
        seq += 1;
    }

    send_msg(stream, &Message::FileEnd { id }).await?;
    Ok(())
}

/// Receive a file, writing to `dest_dir/<offered_name>`.
/// Expects the next message on the stream to be FileOffer for `id`.
pub async fn recv_file(stream: &mut TcpStream, dest_dir: &Path) -> Result<(u64, String, u64)> {
    // Read FileOffer
    let offer = recv_msg(stream).await?;
    let (id, name, size) = match offer {
        Message::FileOffer { id, name, size } => (id, name, size),
        other => anyhow::bail!("expected FileOffer, got {:?}", other),
    };

    let dest = dest_dir.join(&name);
    let mut out = File::create(&dest).await?;
    let mut received: u64 = 0;

    loop {
        let msg = recv_msg(stream).await?;
        match msg {
            Message::FileChunk { id: cid, bytes, .. } if cid == id => {
                out.write_all(&bytes).await?;
                received += bytes.len() as u64;
            }
            Message::FileEnd { id: eid } if eid == id => break,
            other => anyhow::bail!("unexpected during file recv: {:?}", other),
        }
    }

    out.flush().await?;

    if received != size {
        anyhow::bail!("size mismatch: expected {} got {}", size, received);
    }

    Ok((id, name, size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tokio::net::TcpListener;

    async fn file_transfer_test(data: Vec<u8>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Write source file
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("test.bin");
        std::fs::File::create(&src_path).unwrap().write_all(&data).unwrap();

        let src_path_clone = src_path.clone();
        let server = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            send_file(&mut conn, 1, &src_path_clone).await.unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        let (id, name, size) = recv_file(&mut client, dest_dir.path()).await.unwrap();
        assert_eq!(id, 1);
        assert_eq!(name, "test.bin");
        assert_eq!(size, data.len() as u64);

        let received = std::fs::read(dest_dir.path().join("test.bin")).unwrap();
        assert_eq!(received, data);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn small_file() {
        file_transfer_test(b"hello hopper".to_vec()).await;
    }

    #[tokio::test]
    async fn large_file() {
        // 3 MiB — spans multiple chunks
        let data: Vec<u8> = (0u8..=255).cycle().take(3 * 1024 * 1024).collect();
        file_transfer_test(data).await;
    }

    #[tokio::test]
    async fn empty_file() {
        file_transfer_test(vec![]).await;
    }
}
