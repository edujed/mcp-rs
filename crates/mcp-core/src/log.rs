//! File-based logging for the MCP server.
//!
//! Logs are written to `{log_dir}/yyyy-mm-dd.log` (one file per day),
//! appended with a timestamp prefix. The log directory is configured
//! at startup via [`init`].
//!
//! Usage:
//! ```ignore
//! // Once at startup:
//! mcp_core::log::init("/database/logs");
//!
//! // Anywhere:
//! mlog!("connected to {}", db_name);
//! mlog!("ERROR: {e}");
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Initializes the log directory. Call once at startup with a writable
/// directory. Subsequent calls are no-ops (the first value wins).
pub fn init(dir: &str) {
    let _ = LOG_DIR.set(PathBuf::from(dir));
}

/// Returns the configured log directory, if any.
pub fn log_dir() -> Option<&'static PathBuf> {
    LOG_DIR.get()
}

/// Writes a single log line to `{log_dir}/yyyy-mm-dd.log`.
///
/// The line is prefixed with `HH:MM:SS.mmm`. If the log directory is not
/// initialized or the write fails, the message is silently dropped
/// (logging must never crash the server).
pub fn write_line(msg: &str) {
    let Some(dir) = LOG_DIR.get() else {
        return;
    };
    let now = chrono::Local::now();
    let filename = now.format("%Y-%m-%d.log").to_string();
    let path = dir.join(&filename);

    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };

    let ts = now.format("%H:%M:%S%.3f");
    let _ = writeln!(file, "{ts} {msg}");
}

/// Macro for logging to the daily log file.
///
/// ```ignore
/// mlog!("started");
/// mlog!("query took {}ms", elapsed);
/// ```
#[macro_export]
macro_rules! mlog {
    ($($arg:tt)*) => {
        $crate::log::write_line(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_line_no_panic() {
        // Without init, write_line should silently drop the message.
        write_line("test message");
    }
}
