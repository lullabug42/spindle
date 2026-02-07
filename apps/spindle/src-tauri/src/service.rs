//! Service persistence and Tauri command layer.
//!
//! This module loads/saves service config from the database, builds [ServiceManager],
//! and exposes Tauri commands to the frontend (CRUD, reload, group aliases).

use std::{collections::HashMap, ops::DerefMut, sync::Arc};

use serde::Serialize;
use spindle_core::service::ServiceManager;
use sqlx::{Connection, Row, Sqlite, Transaction, pool::PoolConnection};
use tauri::Manager;
use tokio::sync::Mutex;
use tracing::warn;

/// Full service config loaded from the database, used to rebuild [ServiceManager] or return to the frontend.
#[derive(Debug, Serialize)]
pub struct StoredServiceConfig {
    /// Primary key id of the service in the database.
    pub service_id: u32,
    /// Service name.
    pub name: String,
    /// Service version.
    pub version: String,
    /// Executable program path.
    pub program: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional workspace directory.
    pub workspace: Option<String>,
    /// Startup arguments (ordered by arg_idx).
    pub args: Vec<String>,
    /// Database ids of dependency services.
    pub dependency_ids: Vec<u32>,
    /// Group id this service belongs to (matches [ServiceManager] group index).
    pub group_id: u32,
}

/// One row from the `service_config` table: program path, description, workspace.
pub struct ServiceConfigRow {
    /// Executable program path.
    pub program: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional workspace directory.
    pub workspace: Option<String>,
}

/// Queries `name` and `version` for the given service from the `service` table.
///
/// # Arguments
///
/// * `service_id` - Database id of the service.
/// * `db_conn` - Active pool connection to the spindle DB.
///
/// # Returns
///
/// `Some((name, version))` on success, or `None` if not found or on error.
async fn query_service_name_and_version(
    service_id: u32,
    db_conn: &mut PoolConnection<crate::db::SpindleDbType>,
) -> Option<(String, String)> {
    let query_result = sqlx::query(
        "SELECT name, version FROM service
        WHERE id = $1",
    )
    .bind(service_id)
    .fetch_optional(db_conn.deref_mut())
    .await;
    let row = match query_result {
        Ok(Some(row)) => row,
        Ok(None) => return None,
        Err(e) => {
            warn!("error" = ?e, "service_id" = service_id, "Failed to read stored service name and version");
            return None;
        }
    };
    let ret = (row.get("name"), row.get("version"));
    Some(ret)
}

/// Queries program, description, and workspace for the given service from `service_config`.
///
/// # Arguments
///
/// * `service_id` - Database id of the service.
/// * `db_conn` - Active pool connection to the spindle DB.
///
/// # Returns
///
/// `Some(ServiceConfigRow)` on success, or `None` if not found or on error.
async fn query_service_config(
    service_id: u32,
    db_conn: &mut PoolConnection<crate::db::SpindleDbType>,
) -> Option<ServiceConfigRow> {
    let query_result = sqlx::query(
        "SELECT * FROM service_config
        WHERE service_id = $1",
    )
    .bind(service_id)
    .fetch_optional(db_conn.deref_mut())
    .await;
    let row = match query_result {
        Ok(Some(row)) => row,
        Ok(None) => return None,
        Err(e) => {
            warn!("error" = ?e, "service_id" = service_id, "Failed to read stored service config");
            return None;
        }
    };
    let ret = ServiceConfigRow {
        program: row.get("program"),
        description: row.get("description"),
        workspace: row.get("workspace"),
    };
    Some(ret)
}

/// Queries the argument list for the given service from `service_arg` (ordered by arg_idx).
///
/// # Arguments
///
/// * `service_id` - Database id of the service.
/// * `db_conn` - Active pool connection to the spindle DB.
///
/// # Returns
///
/// `Some(args)` on success, or `None` on error.
async fn query_service_args(
    service_id: u32,
    db_conn: &mut PoolConnection<crate::db::SpindleDbType>,
) -> Option<Vec<String>> {
    let query_result = sqlx::query(
        "SELECT arg_idx, value FROM service_arg
        WHERE service_id = $1",
    )
    .bind(service_id)
    .fetch_all(db_conn.deref_mut())
    .await;
    let rows = match query_result {
        Ok(rows) => rows,
        Err(e) => {
            warn!("error" = ?e, "service_id" = service_id, "Failed to read stored service args");
            return None;
        }
    };
    let mut args_with_idx = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<'_, i32, &str>("arg_idx"),
                row.get::<'_, String, &str>("value"),
            )
        })
        .collect::<Vec<_>>();
    args_with_idx.sort_by(|a, b| a.0.cmp(&b.0));
    let ret = args_with_idx.into_iter().map(|(_, arg)| arg).collect();
    Some(ret)
}

/// Queries dependency ids for the given service from `service_dependency`.
///
/// # Arguments
///
/// * `service_id` - Database id of the service.
/// * `db_conn` - Active pool connection to the spindle DB.
///
/// # Returns
///
/// `Some(dependency_ids)` on success, or `None` on error.
async fn query_service_dependency_ids(
    service_id: u32,
    db_conn: &mut PoolConnection<crate::db::SpindleDbType>,
) -> Option<Vec<u32>> {
    let query_result = sqlx::query(
        "SELECT dependency_id FROM service_dependency
        WHERE service_id = $1",
    )
    .bind(service_id)
    .fetch_all(db_conn.deref_mut())
    .await;
    let rows = match query_result {
        Ok(rows) => rows,
        Err(e) => {
            warn!("error" = ?e, "service_id" = service_id, "Failed to read stored service dependencies");
            return None;
        }
    };
    let ret = rows
        .into_iter()
        .map(|row| row.get("dependency_id"))
        .collect();
    Some(ret)
}

/// Queries the group_id for the given service from `service_group_membership`.
///
/// # Arguments
///
/// * `service_id` - Database id of the service.
/// * `db_conn` - Active pool connection to the spindle DB.
///
/// # Returns
///
/// `Some(group_id)` on success, or `None` if not found or on error.
async fn query_service_group_id(
    service_id: u32,
    db_conn: &mut PoolConnection<crate::db::SpindleDbType>,
) -> Option<u32> {
    let query_result = sqlx::query(
        "SELECT group_id FROM service_group_membership
        WHERE service_id = $1",
    )
    .bind(service_id)
    .fetch_optional(db_conn.deref_mut())
    .await;
    let row = match query_result {
        Ok(Some(row)) => row,
        Ok(None) => return None,
        Err(e) => {
            warn!("error" = ?e, "service_id" = service_id, "Failed to read stored service group membership");
            return None;
        }
    };
    let ret = row.get("group_id");
    Some(ret)
}

/// Assembles a full [StoredServiceConfig] from the database by `service_id` (name, version, config, args, deps, group).
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `service_id` - Database id of the service.
///
/// # Returns
///
/// `Some(StoredServiceConfig)` on success, or `None` if any part is missing or on error.
async fn query_stored_service_config(
    app: &tauri::AppHandle,
    service_id: u32,
) -> Option<StoredServiceConfig> {
    let mut db_conn = match crate::db::acquire_spindle_db_conn(app).await {
        Some(conn) => conn,
        None => return None,
    };
    let (name, version) = match query_service_name_and_version(service_id, &mut db_conn).await {
        Some(ret) => ret,
        None => return None,
    };
    let service_config_row = match query_service_config(service_id, &mut db_conn).await {
        Some(row) => row,
        None => return None,
    };
    let args = match query_service_args(service_id, &mut db_conn).await {
        Some(args) => args,
        None => return None,
    };
    let dependency_ids = match query_service_dependency_ids(service_id, &mut db_conn).await {
        Some(ids) => ids,
        None => return None,
    };
    let group_id = match query_service_group_id(service_id, &mut db_conn).await {
        Some(group_id) => group_id,
        None => return None,
    };
    let ret = StoredServiceConfig {
        name,
        version,
        service_id,
        program: service_config_row.program,
        description: service_config_row.description,
        workspace: service_config_row.workspace,
        args,
        dependency_ids,
        group_id,
    };
    Some(ret)
}

/// Queries all service ids from the database.
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
///
/// # Returns
///
/// Vector of service ids; empty on connection failure or query error.
async fn query_all_service_id(app: &tauri::AppHandle) -> Vec<u32> {
    let mut db_conn = match crate::db::acquire_spindle_db_conn(app).await {
        Some(conn) => conn,
        None => return Vec::new(),
    };
    let query_result = sqlx::query("SELECT id FROM service")
        .fetch_all(db_conn.deref_mut())
        .await;
    let rows = match query_result {
        Ok(rows) => rows,
        Err(e) => {
            warn!("error" = ?e, "Failed to query all service id");
            return Vec::new();
        }
    };
    let mut ret = Vec::with_capacity(rows.len());
    for row in rows {
        ret.push(row.get("id"));
    }
    ret
}

/// Queries the database id of a service by (name, version).
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `name` - Service name.
/// * `version` - Service version.
///
/// # Returns
///
/// `Some(service_id)` if found, or `None` if not found or on error.
async fn query_service_id_by_name_and_version(
    app: &tauri::AppHandle,
    name: &str,
    version: &str,
) -> Option<u32> {
    let mut db_conn = match crate::db::acquire_spindle_db_conn(app).await {
        Some(conn) => conn,
        None => return None,
    };
    let query_result = sqlx::query(
        "SELECT id FROM service
        WHERE name = $1 AND version = $2",
    )
    .bind(name)
    .bind(version)
    .fetch_optional(db_conn.deref_mut())
    .await;
    match query_result {
        Ok(Some(row)) => row.get("id"),
        Ok(None) => return None,
        Err(e) => {
            warn!("error" = ?e, "name" = name, "version" = version, "Failed to query service id");
            return None;
        }
    }
}

/// Inserts a new service into the database (service, service_config, service_arg, service_dependency).
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `name` - Service name.
/// * `version` - Service version.
/// * `program` - Executable program path.
/// * `description` - Optional description.
/// * `workspace` - Optional workspace path.
/// * `args` - Startup arguments.
/// * `dependency_ids` - Database ids of dependency services.
///
/// # Returns
///
/// `Ok(service_id)` with the newly assigned id, or an error.
async fn insert_stored_service_config(
    app: &tauri::AppHandle,
    name: &str,
    version: &str,
    program: &str,
    description: Option<&str>,
    workspace: Option<&str>,
    args: &[String],
    dependency_ids: &[u32],
) -> anyhow::Result<u32> {
    let mut db_conn = crate::db::acquire_spindle_db_conn(app)
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to acquire database connection"))?;
    let mut tx = db_conn.begin().await?;
    let service_id =
        match sqlx::query("INSERT INTO service (name, version) VALUES ($1, $2) RETURNING id")
            .bind(name)
            .bind(version)
            .fetch_one(tx.deref_mut())
            .await
        {
            Ok(row) => row.get::<'_, i64, &str>("id") as u32,
            Err(e) => {
                warn!("error" = ?e, "name" = name, "version" = version, "Failed to insert service");
                anyhow::bail!("Failed to insert service");
            }
        };
    sqlx::query(
        "INSERT INTO service_config (service_id, program, description, workspace) VALUES ($1, $2, $3, $4)"
    )
    .bind(service_id)
    .bind(program)
    .bind(description)
    .bind(workspace)
    .execute(tx.deref_mut())
    .await?;
    for (arg_idx, arg) in args.iter().enumerate() {
        sqlx::query("INSERT INTO service_arg (service_id, arg_idx, value) VALUES ($1, $2, $3)")
            .bind(service_id)
            .bind(arg_idx as u32)
            .bind(arg)
            .execute(tx.deref_mut())
            .await?;
    }
    for dependency_id in dependency_ids {
        sqlx::query("INSERT INTO service_dependency (service_id, dependency_id) VALUES ($1, $2)")
            .bind(service_id)
            .bind(dependency_id)
            .execute(tx.deref_mut())
            .await?;
    }
    tx.commit().await?;
    Ok(service_id)
}

/// Removes the service with the given `service_id` from the database.
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `service_id` - Database id of the service to remove.
///
/// # Returns
///
/// `Ok(())` on success; dependent rows are handled by DB constraints.
async fn remove_stored_service_config(
    app: &tauri::AppHandle,
    service_id: u32,
) -> anyhow::Result<()> {
    let mut db_conn = crate::db::acquire_spindle_db_conn(app)
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to acquire database connection"))?;
    let res = sqlx::query("DELETE FROM service WHERE id = ?")
        .bind(service_id as i64)
        .execute(db_conn.deref_mut())
        .await?;
    if res.rows_affected() == 0 {
        warn!("service_id" = service_id, "Service not found");
    }
    Ok(())
}

/// Builds a [ServiceManager] from the given [StoredServiceConfig] list (including dependency name/version mapping).
///
/// # Arguments
///
/// * `configs` - Slice of stored service configs already loaded from the DB.
///
/// # Returns
///
/// `Ok(Arc<ServiceManager>)` on success, or an error.
async fn create_service_manager(
    configs: &[StoredServiceConfig],
) -> anyhow::Result<Arc<ServiceManager>> {
    let service_id_key_map: HashMap<u32, (String, String)> = configs
        .iter()
        .map(|config| {
            (
                config.service_id,
                (config.name.clone(), config.version.clone()),
            )
        })
        .collect();
    let mut service_configs = Vec::with_capacity(configs.len());
    for config in configs {
        let mut dependencies = Vec::with_capacity(config.dependency_ids.len());
        for dependency_id in config.dependency_ids.iter() {
            if let Some((dep_name, dep_version)) = service_id_key_map.get(dependency_id) {
                dependencies.push((dep_name.clone(), dep_version.clone()));
            }
        }
        let service_config = spindle_core::service::ServiceConfig {
            name: config.name.clone(),
            version: config.version.clone(),
            program: config.program.clone().into(),
            args: config.args.clone(),
            dependencies,
            workspace: config.workspace.as_ref().map(|workspace| workspace.into()),
        };
        service_configs.push(service_config);
    }
    Ok(spindle_core::service::ServiceManager::from_configs(
        service_configs,
    ))
}

/// Returns whether `child` is an ordered subsequence of `parent` (used for group matching).
///
/// # Arguments
///
/// * `child` - Candidate subsequence.
/// * `parent` - Candidate supersequence.
///
/// # Returns
///
/// `true` if `child` is an ordered subsequence of `parent`, else `false`.
fn is_subset(child: &[String], parent: &[String]) -> bool {
    let child_len = child.len();
    let parent_len = parent.len();
    let mut child_idx = 0;
    let mut parent_idx = 0;
    while child_idx < child_len && parent_idx < parent_len {
        if child[child_idx] == parent[parent_idx] {
            child_idx += 1;
        }
        parent_idx += 1;
    }
    child_idx == child_len
}

/// Maps aliases from previous groups to current groups by matching service name sets.
///
/// # Arguments
///
/// * `prev` - Previous group alias and service names: (alias, service_names).
/// * `curr` - Current group ids and service names: (group_id, service_names).
///
/// # Returns
///
/// A vector of (group_id, alias) for the current groups that matched.
fn map_alias<'a>(
    prev: &[(&'a str, Vec<String>)],
    curr: &[(u32, Vec<String>)],
) -> Vec<(u32, &'a str)> {
    let mut match_flags = vec![false; curr.len()];
    let mut ret = Vec::with_capacity(curr.len());
    for (alias, prev_service_names) in prev {
        let prev_group_service_num = prev_service_names.len();
        for idx in 0..curr.len() {
            if match_flags[idx] {
                continue;
            }
            let curr_group_id = curr[idx].0;
            let curr_service_names = &curr[idx].1;
            let curr_group_service_num = curr_service_names.len();
            let is_match = {
                if prev_group_service_num <= curr_group_service_num {
                    is_subset(prev_service_names, curr_service_names)
                } else {
                    is_subset(curr_service_names, prev_service_names)
                }
            };
            if is_match {
                match_flags[idx] = true;
                ret.push((curr_group_id, *alias));
            }
        }
    }
    ret
}

/// Within a transaction, migrates group aliases: matches current [ServiceManager] groups to the old `service_group_alias` table and rewrites aliases to new group_ids.
///
/// # Arguments
///
/// * `tx` - Active SQLite transaction.
/// * `service_manager` - Current service manager holding group definitions.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
async fn migrate_group_alias(
    tx: &mut Transaction<'_, Sqlite>,
    service_manager: &ServiceManager,
) -> anyhow::Result<()> {
    let group_aliases: Vec<(String, u32)> =
        sqlx::query("SELECT group_id, alias FROM service_group_alias")
            .fetch_all(tx.deref_mut())
            .await?
            .into_iter()
            .map(|row| (row.get("alias"), row.get("group_id")))
            .collect();
    let mut prev: Vec<(&str, Vec<String>)> = Vec::with_capacity(group_aliases.len());
    for (alias, group_id) in group_aliases.iter() {
        let service_ids: Vec<u32> =
            sqlx::query("SELECT service_id FROM service_group_membership WHERE group_id = $1")
                .bind(*group_id)
                .fetch_all(tx.deref_mut())
                .await?
                .into_iter()
                .map(|row| row.get("service_id"))
                .collect();
        let mut service_names = Vec::with_capacity(service_ids.len());
        for service_id in service_ids {
            let service_name: String = match sqlx::query("SELECT name FROM service WHERE id = $1")
                .bind(service_id)
                .fetch_optional(tx.deref_mut())
                .await
            {
                Ok(Some(row)) => row.get("name"),
                Ok(None) => continue,
                Err(e) => {
                    warn!("error" = ?e, "service_id" = service_id, "Failed to query service name");
                    continue;
                }
            };
            service_names.push(service_name);
        }
        service_names.sort();
        prev.push((alias.as_str(), service_names));
    }
    let curr_group_num = service_manager.group_num();
    let mut curr = Vec::with_capacity(curr_group_num);
    for group_idx in 0..curr_group_num {
        let mut service_names: Vec<String> = service_manager
            .group_service_keys(group_idx)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        service_names.sort();
        curr.push((group_idx as u32, service_names));
    }
    let curr_aliases = map_alias(&prev, &curr);
    sqlx::query("DELETE FROM service_group_alias")
        .execute(tx.deref_mut())
        .await?;
    for (group_id, alias) in curr_aliases {
        sqlx::query("INSERT INTO service_group_alias (group_id, alias) VALUES ($1, $2)")
            .bind(group_id)
            .bind(alias)
            .execute(tx.deref_mut())
            .await?;
    }
    Ok(())
}

/// Updates the database from the current [ServiceManager] groups: migrates aliases, then clears and rewrites `service_group_membership`.
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB and app state access.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
async fn update_service_group_membership(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let app_state = app.state::<Mutex<crate::AppState>>();
    let service_manager = match app_state.lock().await.service_manager.as_ref() {
        Some(sm) => sm.clone(),
        None => anyhow::bail!("Service manager not initialized"),
    };
    let mut db_conn = match crate::db::acquire_spindle_db_conn(app).await {
        Some(conn) => conn,
        None => anyhow::bail!("Failed to acquire database connection"),
    };
    let mut tx = db_conn.begin().await?;
    migrate_group_alias(&mut tx, &service_manager).await?;
    // clear all service group membership
    sqlx::query("DELETE FROM service_group_membership")
        .execute(tx.deref_mut())
        .await?;
    let group_num = service_manager.group_num();
    for group_idx in 0..group_num {
        let services = service_manager.group_service_keys(group_idx);
        for (name, version) in services {
            let service_id = match query_service_id_by_name_and_version(app, &name, &version).await
            {
                Some(id) => id,
                None => {
                    warn!("name" = name, "version" = version, "Service not found");
                    continue;
                }
            };
            // group_idx as group_id
            sqlx::query(
                "INSERT INTO service_group_membership (service_id, group_id) VALUES ($1, $2)",
            )
            .bind(service_id)
            .bind(group_idx as u32)
            .execute(tx.deref_mut())
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Inserts a group alias for the given group_id.
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `group_id` - Group id to attach the alias to.
/// * `alias` - Alias string.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
async fn insert_group_alias(
    app: &tauri::AppHandle,
    group_id: u32,
    alias: &str,
) -> anyhow::Result<()> {
    let mut db_conn = crate::db::acquire_spindle_db_conn(app)
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to acquire database connection"))?;
    sqlx::query("INSERT INTO service_group_alias (group_id, alias) VALUES ($1, $2)")
        .bind(group_id)
        .bind(alias)
        .execute(db_conn.deref_mut())
        .await?;
    Ok(())
}

/// Queries the alias for the given group_id.
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `group_id` - Group id to look up.
///
/// # Returns
///
/// `Some(alias)` if found, or `None` if not found or on error.
async fn query_group_alias(app: &tauri::AppHandle, group_id: u32) -> Option<String> {
    let mut db_conn = match crate::db::acquire_spindle_db_conn(app).await {
        Some(conn) => conn,
        None => return None,
    };
    let query_result = sqlx::query("SELECT alias FROM service_group_alias WHERE group_id = $1")
        .bind(group_id)
        .fetch_optional(db_conn.deref_mut())
        .await;
    match query_result {
        Ok(Some(row)) => Some(row.get("alias")),
        Ok(None) => None,
        Err(e) => {
            warn!("error" = ?e, "group_id" = group_id, "Failed to query group alias");
            None
        }
    }
}

/// Removes the group alias for the given group_id.
///
/// # Arguments
///
/// * `app` - Tauri app handle for DB access.
/// * `group_id` - Group id whose alias to remove.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
async fn remove_group_alias(app: &tauri::AppHandle, group_id: u32) -> anyhow::Result<()> {
    let mut db_conn = crate::db::acquire_spindle_db_conn(app)
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to acquire database connection"))?;
    sqlx::query("DELETE FROM service_group_alias WHERE group_id = $1")
        .bind(group_id)
        .execute(db_conn.deref_mut())
        .await?;
    Ok(())
}

/// Tauri commands exposed to the frontend: service add/remove, reload, group membership and aliases.
pub mod tauri_cmd {
    use tauri::Manager;
    use tokio::sync::Mutex;
    use tracing::{info, warn};

    /// Adds a new service and persists it to the database; dependencies are given as (name, version) and resolved to dependency_ids.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `name` - Service name.
    /// * `version` - Service version.
    /// * `program` - Executable program path.
    /// * `description` - Optional description.
    /// * `workspace` - Optional workspace path.
    /// * `args` - Startup arguments.
    /// * `dependencies` - List of (name, version) for dependencies.
    ///
    /// # Returns
    ///
    /// `Ok(service_id)` with the new service id, or `Err(message)` on failure (e.g. dependency not found).
    #[tauri::command]
    pub async fn add_service(
        app: tauri::AppHandle,
        name: String,
        version: String,
        program: String,
        description: Option<String>,
        workspace: Option<String>,
        args: Vec<String>,
        dependencies: Vec<(String, String)>,
    ) -> Result<u32, String> {
        let mut dependency_ids = Vec::with_capacity(dependencies.len());
        for (dep_name, dep_version) in dependencies {
            match super::query_service_id_by_name_and_version(&app, &dep_name, &dep_version).await {
                Some(dep_id) => dependency_ids.push(dep_id),
                None => {
                    warn!(
                        "dep_name" = dep_name,
                        "dep_version" = dep_version,
                        "Dependency not found"
                    );
                    return Err(format!(
                        "Dependency not found: {}:v{}",
                        dep_name, dep_version
                    ));
                }
            }
        }
        let service_id = super::insert_stored_service_config(
            &app,
            &name,
            &version,
            &program,
            description.as_deref(),
            workspace.as_deref(),
            &args,
            &dependency_ids,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(service_id)
    }

    /// Removes a service by (name, version). Succeeds silently if the service does not exist.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `name` - Service name.
    /// * `version` - Service version.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success or when service not found; `Err(message)` on DB error.
    #[tauri::command]
    pub async fn remove_service(
        app: tauri::AppHandle,
        name: String,
        version: String,
    ) -> Result<(), String> {
        let service_id =
            match super::query_service_id_by_name_and_version(&app, &name, &version).await {
                Some(id) => id,
                None => {
                    info!("name" = name, "version" = version, "Service not found");
                    return Ok(());
                }
            };
        match super::remove_stored_service_config(&app, service_id).await {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!("error" = ?e, "name" = name, "version" = version, "Failed to remove service");
                Err(e.to_string())
            }
        }
    }

    /// Loads all service configs from the database, rebuilds [ServiceManager], and updates app state.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(message)` on failure.
    #[tauri::command]
    pub async fn reload_service_manager(app: tauri::AppHandle) -> Result<(), String> {
        let service_ids = super::query_all_service_id(&app).await;
        let mut configs = Vec::with_capacity(service_ids.len());
        for service_id in service_ids {
            if let Some(config) = super::query_stored_service_config(&app, service_id).await {
                configs.push(config);
            }
        }
        let service_manager = super::create_service_manager(&configs)
            .await
            .map_err(|e| e.to_string())?;
        let app_state = app.state::<Mutex<crate::AppState>>();
        app_state.lock().await.service_manager = Some(service_manager);
        Ok(())
    }

    /// Updates DB group membership and aliases from the current ServiceManager.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(message)` on failure.
    #[tauri::command]
    pub async fn update_service_group_membership(app: tauri::AppHandle) -> Result<(), String> {
        super::update_service_group_membership(&app)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Sets the alias for a group.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `group_id` - Group id.
    /// * `alias` - Alias string.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(message)` on failure.
    #[tauri::command]
    pub async fn insert_group_alias(
        app: tauri::AppHandle,
        group_id: u32,
        alias: String,
    ) -> Result<(), String> {
        super::insert_group_alias(&app, group_id, &alias)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Returns the alias for the given group.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `group_id` - Group id to look up.
    ///
    /// # Returns
    ///
    /// `Some(alias)` if set, or `None` if not set or on error.
    #[tauri::command]
    pub async fn query_group_alias(app: tauri::AppHandle, group_id: u32) -> Option<String> {
        super::query_group_alias(&app, group_id).await
    }

    /// Removes the alias for the given group.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `group_id` - Group id whose alias to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(message)` on failure.
    #[tauri::command]
    pub async fn remove_group_alias(app: tauri::AppHandle, group_id: u32) -> Result<(), String> {
        super::remove_group_alias(&app, group_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Launches all services in the given group.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `group_id` - Group id to launch.
    /// * `timeout_ms` - Max duration to wait for each service to reach Running.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(message)` on failure.
    #[tauri::command]
    pub async fn launch_group(
        app: tauri::AppHandle,
        group_id: usize,
        timeout_ms: u64,
    ) -> Result<(), String> {
        let app_state = app.state::<Mutex<crate::AppState>>();
        let service_manager = match app_state.lock().await.service_manager.as_ref() {
            Some(sm) => sm.clone(),
            None => return Err("Service manager not initialized".to_string()),
        };
        let group_num = service_manager.group_num();
        if group_id >= group_num {
            return Err(format!("Invalid group id: {}", group_id));
        }
        let service_start_timeout = std::time::Duration::from_millis(timeout_ms);
        service_manager
            .launch_group(group_id, service_start_timeout)
            .await
            .map_err(|e| e.to_string())
    }

    /// Stops a service by (name, version).
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `name` - Service name.
    /// * `version` - Service version.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(message)` on failure.
    #[tauri::command]
    pub async fn stop_service(
        app: tauri::AppHandle,
        name: String,
        version: String,
    ) -> Result<(), String> {
        let app_state = app.state::<Mutex<crate::AppState>>();
        let service_manager = match app_state.lock().await.service_manager.as_ref() {
            Some(sm) => sm.clone(),
            None => return Err("Service manager not initialized".to_string()),
        };
        service_manager
            .stop_service(&name, &version)
            .await
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn service_state(
        app: tauri::AppHandle,
        name: String,
        version: String,
    ) -> Result<String, String> {
        let app_state = app.state::<Mutex<crate::AppState>>();
        let service_manager = match app_state.lock().await.service_manager.as_ref() {
            Some(sm) => sm.clone(),
            None => return Err("Service manager not initialized".to_string()),
        };
        let state = service_manager
            .service_state(&name, &version)
            .ok_or("Service not found".to_string())?;
        Ok(state.to_string())
    }
}
