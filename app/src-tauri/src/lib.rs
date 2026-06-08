mod client_loop;
mod commands;
mod server_loop;

use commands::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logs to the console that launched `tauri dev`. Default level info;
    // override with RUST_LOG (e.g. RUST_LOG=kvm_core=debug,hopper=debug).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,kvm_core=info,hopper=info".into()),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let quit = MenuItem::with_id(app, "quit", "Quit Hopper", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;

            TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Hopper KVM")
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::set_role,
            commands::get_peers,
            commands::get_screens,
            commands::set_layout,
            commands::check_permissions,
            commands::request_permission,
            commands::start_engine,
            commands::stop_engine,
            commands::connect_to_peer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running hopper");
}
