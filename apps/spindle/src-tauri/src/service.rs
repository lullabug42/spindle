use std::sync::{Arc, Mutex};

use spindle_core::service::{GroupServiceInfo, ServiceConfig, ServiceManager};
use tauri::State;
use tracing::warn;

use crate::AppState;

async fn all_configs(service_dirs: Vec<String>) -> Vec<(ServiceConfig, String)> {
    let mut ret: Vec<(ServiceConfig, String)> = Vec::new();
    for service_dir in service_dirs {
        let dir_configs: Vec<(ServiceConfig, String)> =
            match spindle_core::service::scan_services(&service_dir).await {
                Ok(dir_configs) => dir_configs,
                Err(e) => {
                    warn!("service_dir" = service_dir, "error" = ?e, "Failed to scan services");
                    continue;
                }
            };
        ret.extend_from_slice(&dir_configs);
    }
    ret
}

async fn reload_service_manager(
    state: State<'_, AppState>,
    service_dirs: Vec<String>,
) -> anyhow::Result<()> {
    let service_manager = ServiceManager::from_configs(all_configs(service_dirs).await);
    let mut service_manager_guard = state
        .service_manager
        .get_or_init(|| Mutex::new(service_manager.clone()))
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock service manager: {}", e))?;
    *service_manager_guard = service_manager;
    Ok(())
}

async fn service_manager(state: State<'_, AppState>) -> anyhow::Result<Arc<ServiceManager>> {
    match state.service_manager.get() {
        Some(x) => {
            let guard = x.lock().map_err(|e| {
                warn!("error" = ?e, "Failed to lock service manager");
                anyhow::anyhow!("Failed to lock service manager: {}", e)
            })?;
            Ok(guard.clone())
        }
        None => {
            anyhow::bail!("Service manager not initialized")
        }
    }
}

async fn service_group_infos(state: State<'_, AppState>) -> anyhow::Result<Vec<GroupServiceInfo>> {
    let service_manager = service_manager(state).await?;
    Ok(service_manager.group_service_infos())
}

async fn service_group_num(state: State<'_, AppState>) -> anyhow::Result<usize> {
    let service_manager = service_manager(state).await?;
    Ok(service_manager.group_num())
}

pub mod tauri_commands {
    use spindle_core::service::GroupServiceInfo;
    use tauri::State;

    use crate::AppState;

    #[tauri::command]
    pub async fn reload_service_manager(
        state: State<'_, AppState>,
        service_dirs: Vec<String>,
    ) -> Result<(), String> {
        super::reload_service_manager(state, service_dirs)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[tauri::command]
    pub async fn service_group_infos(
        state: State<'_, AppState>,
    ) -> Result<Vec<GroupServiceInfo>, String> {
        let infos = super::service_group_infos(state)
            .await
            .map_err(|e| e.to_string())?;
        Ok(infos)
    }

    #[tauri::command]
    pub async fn service_group_num(state: State<'_, AppState>) -> Result<usize, String> {
        let num = super::service_group_num(state)
            .await
            .map_err(|e| e.to_string())?;
        Ok(num)
    }
}
