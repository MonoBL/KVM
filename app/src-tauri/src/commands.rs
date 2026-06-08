use kvm_core::{
    discovery::{Discovery, PeerInfo},
    engine::{Control, EngineState, Role, SharedState},
    layout::Screen,
    tls,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};

// ---- State types exposed to the frontend ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDto {
    pub id: u64,
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenDto {
    pub id: u64,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDto {
    pub role: String,
    pub control: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsDto {
    pub accessibility: bool,
    pub input_monitoring: bool,
}

// ---- App state ----

pub struct AppState {
    pub engine: SharedState,
    pub peers: Arc<Mutex<Vec<PeerDto>>>,
    pub running: Arc<Mutex<bool>>,
    pub discovery: Arc<Mutex<Option<Discovery>>>,
    /// Set to true to stop server/client loops gracefully.
    pub loop_stop: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        let local = Screen {
            id: 0,
            name: hostname(),
            width: primary_display_width(),
            height: primary_display_height(),
            col: 0,
            row: 0,
        };
        Self {
            engine: Arc::new(Mutex::new(EngineState::new_server(local))),
            peers: Arc::new(Mutex::new(vec![])),
            running: Arc::new(Mutex::new(false)),
            discovery: Arc::new(Mutex::new(None)),
            loop_stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ---- Tauri commands ----

#[tauri::command]
pub fn get_status(state: State<AppState>) -> StatusDto {
    let eng = state.engine.lock().unwrap();
    StatusDto {
        role: match eng.role {
            Role::Server => "server".into(),
            Role::Client => "client".into(),
        },
        control: match eng.control {
            Control::Local => "local".into(),
            Control::Remote { peer_id } => format!("remote:{}", peer_id),
        },
        running: *state.running.lock().unwrap(),
    }
}

#[tauri::command]
pub fn set_role(role: String, state: State<AppState>) -> Result<(), String> {
    let mut eng = state.engine.lock().unwrap();
    eng.role = match role.as_str() {
        "server" => Role::Server,
        "client" => Role::Client,
        _ => return Err(format!("unknown role: {}", role)),
    };
    Ok(())
}

#[tauri::command]
pub fn get_peers(state: State<AppState>) -> Vec<PeerDto> {
    state.peers.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_screens(state: State<AppState>) -> Vec<ScreenDto> {
    let eng = state.engine.lock().unwrap();
    eng.screens
        .iter()
        .map(|s| ScreenDto {
            id: s.id,
            name: s.name.clone(),
            width: s.width,
            height: s.height,
            col: s.col,
            row: s.row,
        })
        .collect()
}

#[tauri::command]
pub fn set_layout(screens: Vec<ScreenDto>, state: State<AppState>) -> Result<(), String> {
    let mut eng = state.engine.lock().unwrap();
    eng.screens = screens
        .into_iter()
        .map(|dto| Screen {
            id: dto.id,
            name: dto.name,
            width: dto.width,
            height: dto.height,
            col: dto.col,
            row: dto.row,
        })
        .collect();
    Ok(())
}

#[tauri::command]
pub fn check_permissions() -> PermissionsDto {
    #[cfg(target_os = "macos")]
    {
        PermissionsDto {
            accessibility: macos_accessibility_trusted(),
            input_monitoring: macos_input_monitoring_trusted(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionsDto {
            accessibility: true,
            input_monitoring: true,
        }
    }
}

#[tauri::command]
pub fn request_permission(which: String, _app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match which.as_str() {
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "input_monitoring" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            _ => return Err(format!("unknown permission: {}", which)),
        };
        std::process::Command::new("open").arg(url).spawn().ok();
    }
    Ok(())
}

#[tauri::command]
pub fn start_engine(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut running = state.running.lock().unwrap();
    if *running {
        return Ok(());
    }
    *running = true;
    state.loop_stop.store(false, Ordering::Relaxed);

    // mDNS discovery
    let mut disc = Discovery::new().map_err(|e| e.to_string())?;
    disc.advertise(&hostname(), crate::server_loop::SERVER_PORT)
        .map_err(|e| e.to_string())?;
    let rx = disc.browse().map_err(|e| e.to_string())?;
    *state.discovery.lock().unwrap() = Some(disc);

    // Forward discovered peers to frontend.
    let peers_ref = Arc::clone(&state.peers);
    let app_clone = app.clone();
    std::thread::spawn(move || {
        while let Ok(peer) = rx.recv() {
            let dto = PeerDto {
                id: peer.port as u64,
                name: peer.name.clone(),
                host: peer.host.clone(),
                port: peer.port,
            };
            peers_ref.lock().unwrap().push(dto.clone());
            let _ = app_clone.emit("peer-discovered", dto);
        }
    });

    // Server loop: accept KVM clients.
    let eng = Arc::clone(&state.engine);
    let stop = Arc::clone(&state.loop_stop);
    tokio::spawn(async move {
        crate::server_loop::run_server(eng, stop).await;
    });

    Ok(())
}

#[tauri::command]
pub fn stop_engine(state: State<AppState>) -> Result<(), String> {
    let mut running = state.running.lock().unwrap();
    *running = false;
    state.loop_stop.store(true, Ordering::Relaxed);
    *state.discovery.lock().unwrap() = None;
    Ok(())
}

/// Connect to a peer as client. Spawns client loop in background.
#[tauri::command]
pub fn connect_to_peer(
    host: String,
    port: u16,
    state: State<AppState>,
) -> Result<(), String> {
    let stop = Arc::clone(&state.loop_stop);
    let screen_w = primary_display_width();
    let screen_h = primary_display_height();
    tokio::spawn(async move {
        crate::client_loop::run_client(host, port, screen_w, screen_h, stop).await;
    });
    Ok(())
}

// ---- Helpers ----

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Hopper".into())
}

fn primary_display_width() -> u32 {
    kvm_core::cursor::primary_display_size().0
}

fn primary_display_height() -> u32 {
    kvm_core::cursor::primary_display_size().1
}

#[cfg(target_os = "macos")]
fn macos_accessibility_trusted() -> bool {
    // AXIsProcessTrusted via system call
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to return true")
        .output();
    // A simpler heuristic: try reading from /dev/input — not available on macOS.
    // Real check via AXIsProcessTrusted requires Objective-C FFI.
    // For now return false to always prompt in dev builds.
    false
}

#[cfg(target_os = "macos")]
fn macos_input_monitoring_trusted() -> bool {
    false
}
