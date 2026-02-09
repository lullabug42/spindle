//! Logger initialization using `tracing_subscriber` and `tracing_appender`.
//! Composes console and rolling file output via layers for easy extension.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;
pub use tracing::level_filters::LevelFilter;
pub use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::Layer, prelude::*, registry};

/// Logger initialization configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoggerConfig {
    /// Whether to output to the console.
    pub console: bool,
    /// Minimum level for console output; only used when `console` is `true`.
    #[serde(with = "level_filter_serde")]
    pub console_level: LevelFilter,
    /// Rolling file output config; `None` disables file logging.
    pub file: Option<FileLogConfig>,
}

/// Rolling file log configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileLogConfig {
    /// Directory for log files.
    #[serde(with = "pathbuf_serde")]
    pub directory: PathBuf,
    /// Log file name prefix (no extension). Default is "spindle".
    pub file_name_prefix: String,
    /// Minimum level for file output.
    #[serde(with = "level_filter_serde")]
    pub level: LevelFilter,
}

/// Serialization helper for LevelFilter
mod level_filter_serde {
    use super::LevelFilter;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(filter: &LevelFilter, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let level_str = match *filter {
            LevelFilter::OFF => "OFF",
            LevelFilter::ERROR => "ERROR",
            LevelFilter::WARN => "WARN",
            LevelFilter::INFO => "INFO",
            LevelFilter::DEBUG => "DEBUG",
            LevelFilter::TRACE => "TRACE",
        };
        level_str.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<LevelFilter, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "OFF" => Ok(LevelFilter::OFF),
            "ERROR" => Ok(LevelFilter::ERROR),
            "WARN" => Ok(LevelFilter::WARN),
            "INFO" => Ok(LevelFilter::INFO),
            "DEBUG" => Ok(LevelFilter::DEBUG),
            "TRACE" => Ok(LevelFilter::TRACE),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid level filter: {}",
                s
            ))),
        }
    }
}

/// Serialization helper for PathBuf
mod pathbuf_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::path::PathBuf;

    pub fn serialize<S>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        path.to_string_lossy().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(PathBuf::from(s))
    }
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            console: true,
            console_level: LevelFilter::WARN,
            file: None,
        }
    }
}

impl Default for FileLogConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("."),
            file_name_prefix: "spindle".to_string(),
            level: LevelFilter::INFO,
        }
    }
}

/// Initializes the global logger.
///
/// Adds console and/or rolling file layers from `config`, making it easy to add more layers later.
/// When file output is enabled, the returned `WorkerGuard` must be held by the caller until process
/// exit, or the background writer thread may stop and logs can be lost.
///
/// # Arguments
///
/// * `config` - Logger configuration:
///   - `None`: Load configuration from `tauri_plugin_store` (requires `app_handle`).
///   - `Some(config)`: Use the provided configuration and save it to `tauri_plugin_store` if `app_handle` is provided.
/// * `app_handle` - Optional Tauri app handle for store persistence. If `None`, store operations are skipped.
///
/// # Returns
///
/// * `Ok(Some(guard))` when file logging is enabled; the caller must keep `guard` alive.
/// * `Ok(None)` when only console (or no output) is enabled.
///
/// # Errors
///
/// Returns an error if `config.file` is `Some` and the rolling appender cannot be created in the
/// given directory, or if store operations fail.
pub fn init_logger<R: tauri::Runtime>(
    config: Option<LoggerConfig>,
    app_handle: Option<&tauri::AppHandle<R>>,
) -> anyhow::Result<Option<WorkerGuard>> {
    const STORE_PATH: &str = "spindle-kv-store";
    const STORE_KEY: &str = "logger_config";

    let final_config = match config {
        Some(cfg) => {
            // Save to store if app_handle is available
            if let Some(handle) = app_handle {
                if let Ok(store) = handle.store(STORE_PATH) {
                    let json_value = serde_json::to_value(&cfg)
                        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
                    store.set(STORE_KEY, json_value);
                    if let Err(e) = store.save() {
                        tracing::warn!("Failed to save logger config to store: {}", e);
                    }
                }
            }
            cfg
        }
        None => {
            // Load from store if app_handle is available
            if let Some(handle) = app_handle {
                if let Ok(store) = handle.store(STORE_PATH) {
                    if let Some(json_value) = store.get(STORE_KEY) {
                        match serde_json::from_value::<LoggerConfig>(json_value) {
                            Ok(cfg) => cfg,
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to deserialize logger config from store: {}, using default",
                                    e
                                );
                                LoggerConfig::default()
                            }
                        }
                    } else {
                        tracing::info!("No logger config found in store, using default");
                        LoggerConfig::default()
                    }
                } else {
                    tracing::warn!("Failed to load store, using default logger config");
                    LoggerConfig::default()
                }
            } else {
                tracing::warn!(
                    "No app_handle provided and config is None, using default logger config"
                );
                LoggerConfig::default()
            }
        }
    };

    init_logger_with_config(final_config)
}

/// Internal function that initializes the logger with a specific configuration.
fn init_logger_with_config(config: LoggerConfig) -> anyhow::Result<Option<WorkerGuard>> {
    let mut guard = None;
    let mut layers: Vec<Box<dyn Layer<registry::Registry> + Send + Sync>> = Vec::new();

    if config.console {
        let layer = fmt::layer()
            .pretty()
            .with_timer(fmt::time::UtcTime::rfc_3339())
            .with_writer(std::io::stdout)
            .with_filter(config.console_level)
            .boxed();
        layers.push(layer);
    }

    if let Some(ref file_cfg) = config.file {
        let file_appender = rolling::daily(&file_cfg.directory, &file_cfg.file_name_prefix);
        let (file_writer, worker_guard) = tracing_appender::non_blocking(file_appender);
        guard = Some(worker_guard);
        let layer = fmt::layer()
            .json()
            .with_writer(file_writer)
            .with_ansi(false)
            .with_timer(fmt::time::UtcTime::rfc_3339())
            .with_filter(file_cfg.level)
            .boxed();
        layers.push(layer);
    }

    let combined = layers
        .into_iter()
        .reduce(|acc, layer| acc.and_then(layer).boxed());

    match combined {
        None => registry::Registry::default().init(),
        Some(layer) => registry::Registry::default().with(layer).init(),
    }

    Ok(guard)
}
