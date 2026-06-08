use kvm_core::{
    capture::{run_server_grab_loop, ServerGrabEvent},
    cursor::{set_visible_sync, warp_sync},
    engine::{MouseOutcome, SharedState},
    layout::Screen,
    protocol::Message,
    session::server_handshake,
    transport::{recv_msg, send_msg, split},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc;

pub const SERVER_PORT: u16 = 15900;

/// Accept loop: listens on SERVER_PORT and handles each client in a task.
/// Exits when `stop` is set (polled every 100 ms).
pub async fn run_server(engine: SharedState, stop: Arc<AtomicBool>) {
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", SERVER_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("server bind :{}: {}", SERVER_PORT, e);
            return;
        }
    };
    tracing::info!("Hopper server on :{}", SERVER_PORT);

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            res = listener.accept() => match res {
                Ok((tcp, addr)) => {
                    tracing::info!("client from {}", addr);
                    let eng = Arc::clone(&engine);
                    let st = Arc::clone(&stop);
                    tokio::spawn(async move {
                        if let Err(e) = peer_session(tcp, eng, st).await {
                            tracing::warn!("peer ended: {}", e);
                        }
                    });
                }
                Err(e) => tracing::warn!("accept: {}", e),
            },
            // Poll stop flag even when no client connects.
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
}

/// Per-client session: runs grab loop + TCP send/recv for one peer.
async fn peer_session(
    stream: tokio::net::TcpStream,
    engine: SharedState,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let session = server_handshake(stream, "hopper").await?;

    // Extract fields before stream is consumed by split().
    let peer_id = session.peer_id;
    let peer_name = session.peer_name.clone();
    let screen_w = session.screen_w;
    let screen_h = session.screen_h;
    let (mut writer, mut reader) = split(session.stream);

    // B2: register peer screen so neighbor() can find it.
    // Place to the right of the local screen unless already positioned by the
    // layout editor (i.e., a screen with this id already exists).
    {
        let mut eng = engine.lock().unwrap();
        if !eng.screens.iter().any(|s| s.id == peer_id) {
            let (col, row) = eng.screens.first()
                .map(|s| (s.col + 1, s.row))
                .unwrap_or((1, 0));
            eng.add_screen(Screen {
                id: peer_id,
                name: peer_name,
                width: screen_w,
                height: screen_h,
                col,
                row,
            });
        }
    }
    tracing::info!("peer {} registered ({}x{})", peer_id, screen_w, screen_h);

    // mpsc bridges sync grab thread -> async writer.
    let (tx, mut rx) = mpsc::channel::<Message>(512);

    // Grab thread: owns rdev::grab; warp/hide via Send-safe FFI helpers.
    {
        let eng_grab = Arc::clone(&engine);
        let tx_grab = tx; // moved; drop closes channel when thread exits
        let stop_grab = Arc::clone(&stop);
        std::thread::spawn(move || {
            run_server_grab_loop(move |ev| {
                if stop_grab.load(Ordering::Relaxed) {
                    return false; // pass event through; we're done
                }
                let outcome = {
                    let mut eng = eng_grab.lock().unwrap();
                    match ev {
                        ServerGrabEvent::MouseAbs(x, y) => eng.on_mouse_abs(x, y),
                        ServerGrabEvent::Msg(msg) => {
                            let fwd = eng.forward(msg);
                            MouseOutcome {
                                messages: fwd.into_iter().collect(),
                                ..Default::default()
                            }
                        }
                    }
                };
                // Apply warp/hide via Send-safe FFI (no Enigo needed).
                if let Some((x, y)) = outcome.warp {
                    warp_sync(x, y);
                }
                if let Some(hide) = outcome.set_hidden {
                    set_visible_sync(!hide);
                }
                let suppress = !outcome.messages.is_empty() || outcome.warp.is_some();
                for msg in outcome.messages {
                    // try_send: drop rather than block grab loop.
                    let _ = tx_grab.try_send(msg);
                }
                suppress
            });
        });
    }

    // Recv task: client -> server (LeaveScreen, Heartbeat).
    let eng_recv = Arc::clone(&engine);
    let stop_recv = Arc::clone(&stop);
    let recv_handle = tokio::spawn(async move {
        while !stop_recv.load(Ordering::Relaxed) {
            match recv_msg(&mut reader).await {
                Ok(Message::LeaveScreen) => {
                    eng_recv.lock().unwrap().on_leave_screen();
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    // Send loop: grab events -> TCP writer.
    while let Some(msg) = rx.recv().await {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Err(e) = send_msg(&mut writer, &msg).await {
            tracing::warn!("send to client: {}", e);
            break;
        }
    }

    recv_handle.abort();

    // B2: unregister peer on disconnect; reset control if we were remote.
    engine.lock().unwrap().remove_peer(peer_id);
    tracing::info!("peer {} unregistered", peer_id);

    Ok(())
}
