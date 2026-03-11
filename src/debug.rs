use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

const LOG_FILE: &str = "spixelatuir-debug.log";

/// Simple file-based logger for TUI applications (stdout/stderr are owned by the terminal).
struct FileLogger {
    file: Mutex<File>,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata())
            && let Ok(mut f) = self.file.lock()
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let _ = writeln!(
                f,
                "[{:.3}] {} [{}] {}",
                now.as_secs_f64(),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

/// Initialise the file logger. Writes to `spixelatuir-debug.log` in the current directory.
/// Returns `Ok(())` on success, or an error if the log file cannot be opened.
pub fn init() -> anyhow::Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)
        .map_err(|e| anyhow::anyhow!("Failed to open log file {LOG_FILE}: {e}"))?;

    let logger = FileLogger {
        file: Mutex::new(file),
    };

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(LevelFilter::Debug))
        .map_err(|e| anyhow::anyhow!("Failed to set logger: {e}"))?;

    log::info!("Debug logging initialised — writing to {LOG_FILE}");
    Ok(())
}
