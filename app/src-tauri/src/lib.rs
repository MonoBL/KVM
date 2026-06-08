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
