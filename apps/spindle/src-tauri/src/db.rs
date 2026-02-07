use sqlx::{Sqlite, pool::PoolConnection};
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool, Migration, MigrationKind};
use tracing::error;

pub const SPINDLE_DB_URL: &str = "sqlite:spindle.db";
pub type SpindleDbType = Sqlite;

const SPINDLE_MIGRATION_1: &str = r##"CREATE TABLE IF NOT EXISTS service (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    version     TEXT NOT NULL,
    UNIQUE (name, version)
);

CREATE TABLE IF NOT EXISTS service_config (
    service_id  INTEGER PRIMARY KEY,
    program     TEXT NOT NULL,
    description TEXT,
    workspace   TEXT,
    CONSTRAINT fk_service_config_service_id
        FOREIGN KEY (service_id) REFERENCES service (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS service_arg (
    service_id INTEGER NOT NULL,
    arg_idx    INTEGER NOT NULL,
    value      TEXT NOT NULL,
    CONSTRAINT fk_service_arg_service_id
        FOREIGN KEY (service_id) REFERENCES service (id) ON DELETE CASCADE,
    PRIMARY KEY (service_id, arg_idx),
    CHECK (arg_idx >= 0)
);

CREATE TABLE IF NOT EXISTS service_dependency (
    service_id    INTEGER NOT NULL,
    dependency_id INTEGER NOT NULL,
    CONSTRAINT fk_service_dependency_service_id
        FOREIGN KEY (service_id) REFERENCES service (id) ON DELETE CASCADE,
    CONSTRAINT fk_service_dependency_dependency_id
        FOREIGN KEY (dependency_id) REFERENCES service (id) ON DELETE RESTRICT,
    PRIMARY KEY (service_id, dependency_id),
    CHECK (service_id != dependency_id)
);
CREATE INDEX IF NOT EXISTS idx_service_dependency_dependency_id ON service_dependency (dependency_id);

CREATE TABLE IF NOT EXISTS service_group_membership (
    service_id INTEGER PRIMARY KEY,
    group_id   INTEGER NOT NULL,
    CONSTRAINT fk_service_group_membership_service_id
        FOREIGN KEY (service_id) REFERENCES service (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_service_group_membership_group_id ON service_group_membership (group_id);

CREATE TABLE IF NOT EXISTS service_group_alias (
    group_id INTEGER NOT NULL PRIMARY KEY,
    alias    TEXT NOT NULL UNIQUE
);"##;

pub fn spindle_migrations() -> Vec<Migration> {
    let ret = vec![Migration {
        version: 1,
        description: "initial service table",
        sql: SPINDLE_MIGRATION_1,
        kind: MigrationKind::Up,
    }];
    ret
}

pub async fn acquire_spindle_db_conn<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PoolConnection<SpindleDbType>> {
    let map_guard = app.state::<DbInstances>().inner().0.read().await;
    let db_conn = match map_guard.get(SPINDLE_DB_URL) {
        Some(DbPool::Sqlite(pool)) => match pool.acquire().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("error" = ?e, "Failed to acquire database connection");
                return None;
            }
        },
        None => {
            error!("db_url" = SPINDLE_DB_URL, "Database not found");
            return None;
        }
    };
    Some(db_conn)
}
