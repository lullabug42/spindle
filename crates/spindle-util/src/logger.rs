use std::path::Path;

use tracing::{Level, level_filters::LevelFilter};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{fmt, prelude::*, registry};

pub async fn init_logger(
    console_level: Level,
    file_level: Level,
    log_file: &str,
) -> anyhow::Result<WorkerGuard> {
    let stdout_layer = fmt::layer()
        .pretty()
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_line_number(false)
        .with_file(false)
        .with_writer(std::io::stdout)
        .with_filter(LevelFilter::from_level(console_level));

    let path = Path::new(log_file);
    let log_dir = path.parent().unwrap_or(Path::new("./"));
    let file_name_prefix = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => {
            eprintln!("Failed to get file name from path '{}'", path.display());
            anyhow::bail!("Failed to get log file name");
        }
    };
    let file_appender = rolling::daily(log_dir, file_name_prefix);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .json()
        .with_ansi(false)
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_filter(LevelFilter::from_level(file_level));

    registry().with(stdout_layer).with(file_layer).init();
    Ok(guard)
}
