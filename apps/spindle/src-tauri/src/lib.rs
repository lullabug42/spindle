use std::sync::{Arc, Mutex, OnceLock};

use spindle_core::service::ServiceManager;

mod service;

struct AppState {
    service_manager: OnceLock<Mutex<Arc<ServiceManager>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            service_manager: OnceLock::new(),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            service::tauri_commands::reload_service_manager,
            service::tauri_commands::service_group_infos
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
