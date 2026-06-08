use kvm_core::{
    cursor::Cursor,
    inject::Injector,
    protocol::Message,
    session::client_handshake,
    transport::{recv_msg, send_msg, split},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Connect to a server and run the client injection loop until `stop` is set
/// or the connection drops.
pub async fn run_client(
    host: String,
    port: u16,
    screen_w: u32,
    screen_h: u32,
    stop: Arc<AtomicBool>,
) {
    let session =
        match client_handshake(&host, port, "hopper-client", screen_w, screen_h).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("connect to {}:{}: {}", host, port, e);
                return;
            }
        };
    tracing::info!("connected to {} (peer_id={})", host, session.peer_id);

    let (mut writer, mut reader) = split(session.stream);

    // Sync channel: async recv -> inject thread.
    // Dropping inject_tx closes the channel, which stops inject_thread.
    let (inject_tx, inject_rx) = std::sync::mpsc::sync_channel::<Message>(512);

    // Inject thread: owns Injector + Cursor (not reliably Send on macOS).
    std::thread::spawn(move || inject_thread(inject_rx, screen_w, screen_h));

    // Heartbeat: keeps the connection alive and detects silent drops.
    let stop_hb = Arc::clone(&stop);
    let hb_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        while !stop_hb.load(Ordering::Relaxed) {
            interval.tick().await;
            if let Err(e) = send_msg(&mut writer, &Message::Heartbeat).await {
                tracing::warn!("heartbeat failed: {}", e);
                break;
            }
        }
    });

    // Main recv loop.
    while !stop.load(Ordering::Relaxed) {
        match recv_msg(&mut reader).await {
            Ok(msg) => {
                if inject_tx.try_send(msg).is_err() {
                    tracing::warn!("inject queue full; dropping frame");
                }
            }
            Err(e) => {
                tracing::warn!("server gone: {}", e);
                break;
            }
        }
    }

    // Closing inject_tx stops inject_thread via channel-closed.
    drop(inject_tx);
    hb_handle.abort();
}

/// Blocking injection loop. Runs on a dedicated std::thread.
fn inject_thread(rx: std::sync::mpsc::Receiver<Message>, screen_w: u32, screen_h: u32) {
    let mut injector = match Injector::new() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("injector init: {}", e);
            return;
        }
    };
    let mut cursor = Cursor::new().ok();
    let mut active = false;

    for msg in rx {
        match &msg {
            Message::EnterScreen { entry_x_ratio, entry_y_ratio, .. } => {
                active = true;
                // Position cursor at the entry ratio on this screen.
                if let Some(ref mut c) = cursor {
                    let x = (entry_x_ratio * screen_w as f32) as i32;
                    let y = (entry_y_ratio * screen_h as f32) as i32;
                    if let Err(e) = c.warp(x, y) {
                        tracing::warn!("cursor warp: {}", e);
                    }
                }
            }
            Message::LeaveScreen => {
                active = false;
            }
            _ => {
                if active {
                    if let Err(e) = injector.replay(&msg) {
                        tracing::warn!("inject replay: {}", e);
                    }
                }
            }
        }
    }
}
