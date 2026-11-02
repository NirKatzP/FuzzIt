// Prevents an extra console window from popping up on Windows release
// builds (the window is pointless for a GUI app and undercuts the
// "lightweight, polished" goal).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod fuzzer;

use commands::ActiveRun;
use tokio::sync::Mutex;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ActiveRun(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            commands::start_fuzz,
            commands::stop_fuzz,
            commands::list_bundled_wordlists,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the FuzzIt application");
}
