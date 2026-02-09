use std::sync::Arc;

use spindle_core::service::ServiceManager;
use tauri::Manager;
use tokio::sync::Mutex;

mod db;
mod logger;
mod service;

struct AppState {
    service_manager: Option<Arc<ServiceManager>>, // lazy init
    _logger_guard: Option<logger::WorkerGuard>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            service_manager: None,
            _logger_guard: None,
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:spindle.db", db::spindle_migrations())
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(AppState::default()))
        .setup(|app| {
            let worker_guard = logger::init_logger(None, Some(app.handle()))
                .inspect_err(|e| eprintln!("logger init failed: {:?}", e))
                .unwrap_or(None);

            // Store logger guard in AppState
            let app_state = app.state::<Mutex<AppState>>();
            let mut state = app_state.blocking_lock();
            state._logger_guard = worker_guard;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            service::tauri_cmd::add_service,
            service::tauri_cmd::remove_service,
            service::tauri_cmd::reload_service_manager,
            service::tauri_cmd::update_service_group_membership,
            service::tauri_cmd::insert_group_alias,
            service::tauri_cmd::query_group_alias,
            service::tauri_cmd::remove_group_alias,
            service::tauri_cmd::launch_group,
            service::tauri_cmd::stop_service,
            service::tauri_cmd::service_state,
            service::tauri_cmd::stop_group,
            service::tauri_cmd::aliased_group_service,
            service::tauri_cmd::unaliased_group_service,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
