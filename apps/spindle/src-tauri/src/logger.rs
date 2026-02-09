//! Logger initialization using `tracing_subscriber` and `tracing_appender`.
//! Composes console and rolling file output via layers for easy extension.

use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use tauri_plugin_store::StoreExt;
use tokio::sync::broadcast;
pub use tracing::level_filters::LevelFilter;
use tracing::{Event, Level, Subscriber};
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
    /// Broadcast output config; `None` disables broadcast logging.
    pub broadcast: Option<BroadcastLogConfig>,
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

/// Broadcast log configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BroadcastLogConfig {
    /// Minimum level for broadcast output.
    #[serde(with = "level_filter_serde")]
    pub level: LevelFilter,
    /// Capacity of the broadcast channel. Default is 128.
    pub capacity: usize,
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
            broadcast: None,
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

impl Default for BroadcastLogConfig {
    fn default() -> Self {
        Self {
            level: LevelFilter::INFO,
            capacity: 128,
        }
    }
}

/// Result of logger initialization.
pub struct LoggerInitResult {
    /// Worker guard for file logging, if enabled. Must be kept alive.
    pub worker_guard: Option<WorkerGuard>,
    /// Broadcast receiver for log messages, if broadcast logging is enabled.
    pub broadcast_receiver: Option<broadcast::Receiver<String>>,
}

/// Saves logger configuration to store.
fn save_config_to_store(
    config: &LoggerConfig,
    app_handle: &tauri::AppHandle,
) -> anyhow::Result<()> {
    const STORE_PATH: &str = "spindle-kv-store";
    const STORE_KEY: &str = "logger_config";

    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| anyhow::anyhow!("Failed to access store: {}", e))?;

    let json_value = serde_json::to_value(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    store.set(STORE_KEY, json_value);
    store
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;

    Ok(())
}

/// Loads logger configuration from store.
fn load_config_from_store(app_handle: &tauri::AppHandle) -> Option<LoggerConfig> {
    const STORE_PATH: &str = "spindle-kv-store";
    const STORE_KEY: &str = "logger_config";

    let store = app_handle.store(STORE_PATH).ok()?;
    let json_value = store.get(STORE_KEY)?;

    match serde_json::from_value::<LoggerConfig>(json_value) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!(
                "Failed to deserialize logger config from store: {}, using default",
                e
            );
            None
        }
    }
}

/// Initializes the global logger.
///
/// Adds console and/or rolling file layers from `config`, and optionally a broadcast layer.
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
/// A `LoggerInitResult` containing:
/// * `worker_guard` - `Some(guard)` when file logging is enabled; the caller must keep `guard` alive.
/// * `broadcast_receiver` - `Some(receiver)` when broadcast logging is enabled; use this to receive log messages.
///
/// # Errors
///
/// Returns an error if `config.file` is `Some` and the rolling appender cannot be created in the
/// given directory, or if store operations fail.
pub fn init_logger(
    config: Option<LoggerConfig>,
    app_handle: Option<&tauri::AppHandle>,
) -> anyhow::Result<LoggerInitResult> {
    let final_config = if let Some(cfg) = config {
        // Save to store if app_handle is available
        if let Some(handle) = app_handle {
            if let Err(e) = save_config_to_store(&cfg, handle) {
                tracing::warn!("Failed to save logger config to store: {}", e);
            }
        }
        cfg
    } else {
        // Load from store if app_handle is available
        if let Some(handle) = app_handle {
            load_config_from_store(handle).unwrap_or_else(|| {
                tracing::info!("No logger config found in store, using default");
                LoggerConfig::default()
            })
        } else {
            tracing::warn!(
                "No app_handle provided and config is None, using default logger config"
            );
            LoggerConfig::default()
        }
    };

    init_logger_with_config(final_config)
}

/// Creates console layer if enabled.
fn create_console_layer(
    config: &LoggerConfig,
) -> Option<Box<dyn Layer<registry::Registry> + Send + Sync>> {
    if !config.console {
        return None;
    }

    Some(
        fmt::layer()
            .pretty()
            .with_timer(fmt::time::UtcTime::rfc_3339())
            .with_writer(std::io::stdout)
            .with_filter(config.console_level)
            .boxed(),
    )
}

/// Creates file layer if configured.
fn create_file_layer(
    file_cfg: &FileLogConfig,
) -> anyhow::Result<(
    Box<dyn Layer<registry::Registry> + Send + Sync>,
    WorkerGuard,
)> {
    let file_appender = rolling::daily(&file_cfg.directory, &file_cfg.file_name_prefix);
    let (file_writer, worker_guard) = tracing_appender::non_blocking(file_appender);
    let layer = fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_filter(file_cfg.level)
        .boxed();

    Ok((layer, worker_guard))
}

/// Creates broadcast layer if configured.
fn create_broadcast_layer(
    broadcast_cfg: &BroadcastLogConfig,
) -> (
    Box<dyn Layer<registry::Registry> + Send + Sync>,
    broadcast::Receiver<String>,
) {
    let (broadcast_layer, receiver) =
        BroadcastLayer::new(broadcast_cfg.level, broadcast_cfg.capacity);
    let layer: Box<dyn Layer<registry::Registry> + Send + Sync> = Box::new(broadcast_layer);
    (layer, receiver)
}

/// Internal function that initializes the logger with a specific configuration.
fn init_logger_with_config(config: LoggerConfig) -> anyhow::Result<LoggerInitResult> {
    let mut guard = None;
    let mut layers: Vec<Box<dyn Layer<registry::Registry> + Send + Sync>> = Vec::new();

    // Add console layer
    if let Some(layer) = create_console_layer(&config) {
        layers.push(layer);
    }

    // Add file layer
    if let Some(ref file_cfg) = config.file {
        let (layer, worker_guard) = create_file_layer(file_cfg)?;
        guard = Some(worker_guard);
        layers.push(layer);
    }

    // Add broadcast layer
    let mut broadcast_receiver = None;
    if let Some(ref broadcast_cfg) = config.broadcast {
        let (layer, receiver) = create_broadcast_layer(broadcast_cfg);
        broadcast_receiver = Some(receiver);
        layers.push(layer);
    }

    // Combine all layers and initialize
    let combined = layers
        .into_iter()
        .reduce(|acc, layer| acc.and_then(layer).boxed());

    match combined {
        None => registry::Registry::default().init(),
        Some(layer) => registry::Registry::default().with(layer).init(),
    }

    Ok(LoggerInitResult {
        worker_guard: guard,
        broadcast_receiver,
    })
}

/// A tracing layer that broadcasts log events via a tokio broadcast channel.
///
/// This layer intercepts log events at or above the specified level and sends them
/// through a broadcast channel as pretty-formatted strings. Multiple receivers can subscribe to receive these logs.
pub struct BroadcastLayer {
    sender: broadcast::Sender<String>,
    level: LevelFilter,
}

impl BroadcastLayer {
    /// Creates a new `BroadcastLayer` with the specified level filter and channel capacity.
    ///
    /// # Arguments
    ///
    /// * `level` - Minimum log level to intercept and broadcast
    /// * `capacity` - Capacity of the broadcast channel (default: 1000)
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - The layer instance
    /// - A receiver handle to subscribe to log messages
    pub fn new(level: LevelFilter, capacity: usize) -> (Self, broadcast::Receiver<String>) {
        let (sender, receiver) = broadcast::channel(capacity);
        (Self { sender, level }, receiver)
    }
}

impl<S> Layer<S> for BroadcastLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = *metadata.level();

        // Check if the event level matches our filter
        // Convert LevelFilter to Level for comparison
        let filter_level = match self.level {
            LevelFilter::OFF => return,
            LevelFilter::ERROR => Level::ERROR,
            LevelFilter::WARN => Level::WARN,
            LevelFilter::INFO => Level::INFO,
            LevelFilter::DEBUG => Level::DEBUG,
            LevelFilter::TRACE => Level::TRACE,
        };

        // Only process events at or above the filter level
        // In tracing, Level order is: ERROR < WARN < INFO < DEBUG < TRACE
        // LevelFilter::INFO means "INFO and above" (INFO, WARN, ERROR)
        // So we check if level <= filter_level
        if level > filter_level {
            return;
        }

        // Format the event as a pretty string
        let mut formatted = String::new();

        // Format timestamp (UTC RFC3339)
        let now = Utc::now();
        if write!(formatted, "{} ", now.to_rfc3339()).is_err() {
            eprintln!("BroadcastLayer: Failed to write timestamp");
            return;
        }

        // Format level
        let level_str = match level {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN ",
            Level::INFO => "INFO ",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };
        if write!(formatted, "{:5} ", level_str).is_err() {
            eprintln!("BroadcastLayer: Failed to write level");
            return;
        }

        // Format target/module path
        if write!(formatted, "{}: ", metadata.target()).is_err() {
            eprintln!("BroadcastLayer: Failed to write target");
            return;
        }

        // Collect and format fields
        let mut fields = Vec::new();
        let mut message = String::new();

        event.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                let field_name = field.name();
                let field_value = format!("{:?}", value);

                if field_name == "message" {
                    message = field_value;
                } else {
                    fields.push((field_name.to_string(), field_value));
                }
            },
        );

        // Add message
        if !message.is_empty() {
            if write!(formatted, "{}", message).is_err() {
                eprintln!("BroadcastLayer: Failed to write message");
                return;
            }
        } else {
            if write!(formatted, "{}", metadata.name()).is_err() {
                eprintln!("BroadcastLayer: Failed to write event name");
                return;
            }
        }

        // Add additional fields
        if !fields.is_empty() {
            if write!(formatted, " ").is_err() {
                eprintln!("BroadcastLayer: Failed to write field separator");
                return;
            }
            for (i, (key, value)) in fields.iter().enumerate() {
                if i > 0 {
                    if write!(formatted, ", ").is_err() {
                        eprintln!("BroadcastLayer: Failed to write field separator");
                        return;
                    }
                }
                if write!(formatted, "{}={}", key, value).is_err() {
                    eprintln!("BroadcastLayer: Failed to write field");
                    return;
                }
            }
        }

        // Add newline
        if writeln!(formatted).is_err() {
            eprintln!("BroadcastLayer: Failed to write newline");
            return;
        }

        // Send formatted message
        if let Err(e) = self.sender.send(formatted) {
            eprintln!(
                "BroadcastLayer: Failed to send log message to broadcast channel: {}",
                e
            );
        }
    }

    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        let level = *metadata.level();
        let filter_level = match self.level {
            LevelFilter::OFF => return false,
            LevelFilter::ERROR => Level::ERROR,
            LevelFilter::WARN => Level::WARN,
            LevelFilter::INFO => Level::INFO,
            LevelFilter::DEBUG => Level::DEBUG,
            LevelFilter::TRACE => Level::TRACE,
        };
        level <= filter_level
    }
}

pub mod tauri_cmd {
    use tauri::{Emitter, Manager};
    use tokio::sync::Mutex;

    /// Subscribes to log events and emits them to the frontend.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri app handle.
    /// * `event_name` - Event name to emit logs to.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the log subscription is successful, `Err(String)` if the logger is not initialized.
    ///
    /// # Note
    ///
    /// Each call to this function creates a new subscription task. If called multiple times,
    /// multiple tasks will be spawned, each forwarding logs to the frontend. The tasks will
    /// run until the broadcast channel is closed or the application exits.
    #[tauri::command]
    pub async fn subscribe_log(app: tauri::AppHandle, event_name: String) -> Result<(), String> {
        let app_state = app.state::<Mutex<crate::AppState>>();
        let mut receiver = app_state
            .lock()
            .await
            .logger_broadcast_receiver
            .as_ref()
            .map(|r| r.resubscribe())
            .ok_or_else(|| "Logger not initialized".to_string())?;

        let fut = async move {
            loop {
                match receiver.recv().await {
                    Ok(message) => {
                        if let Err(e) = app.emit(&event_name, message) {
                            eprintln!("Failed to emit log event '{}': {}", event_name, e);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        eprintln!("Broadcast channel closed, stopping log subscription");
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        };

        tokio::spawn(fut);
        Ok(())
    }
}
