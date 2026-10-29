//! Common utilities shared by all tool handlers.
//!
//! These helpers are transport-agnostic and database-agnostic. They
//! operate on JSON arguments and the platform's `Config`/`FirebirdClient`
//! types, making them reusable across any MCP server built on this
//! platform.

use mcp_core::{Config, FirebirdClient, mlog};
use mcp_types::{QueryResult, Value, format_cell};
use serde_json::Value as JsonValue;

/// Extracts a string argument from the JSON args.
pub fn arg_str(args: &JsonValue, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extracts a boolean argument from the JSON args.
pub fn arg_bool(args: &JsonValue, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

/// Extracts a non-negative integer argument, accepting either a JSON number
/// or a numeric string. LLMs frequently send numbers where the schema says
/// string (and vice-versa), so we accept both to avoid spurious failures.
pub fn arg_u32(args: &JsonValue, key: &str) -> Option<u32> {
    match args.get(key)? {
        JsonValue::Number(n) => n.as_u64().and_then(|v| u32::try_from(v).ok()),
        JsonValue::String(s) => s.trim().parse::<u32>().ok(),
        _ => None,
    }
}

/// Opens a client for the requested database (or the default).
pub fn open_client(cfg: &Config, args: &JsonValue) -> Result<FirebirdClient, String> {
    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    mlog!("connecting to '{}' at {}", db_name, db.path);
    FirebirdClient::new(&cfg.server, &db.path).map_err(|e| {
        mlog!("connection to '{}' FAILED: {e}", db_name);
        e.to_string()
    })
}

/// Renders a query result as a Markdown table.
pub fn result_to_text(result: &QueryResult) -> String {
    let mut b = String::new();
    b.push_str(&format!(
        "### Query Results ({} rows)\n\n",
        result.rows.len()
    ));
    if result.columns.is_empty() {
        b.push_str("No columns returned.\n");
        return b;
    }
    let cols = result
        .columns
        .iter()
        .map(|c| format!("{} ({})", c.name, c.type_name))
        .collect::<Vec<_>>()
        .join(", ");
    b.push_str(&format!("Columns: {cols}\n\n"));
    b.push_str(
        &result
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(" | "),
    );
    b.push('\n');
    b.push_str(
        &result
            .columns
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | "),
    );
    b.push('\n');
    for row in &result.rows {
        let cells: Vec<String> = row.iter().map(format_cell).collect();
        b.push_str(&cells.join(" | "));
        b.push('\n');
    }
    b
}

/// Returns "" if n is 1, "s" otherwise.
pub fn plural_suffix(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Gets a cell value as text, or "-" if not present.
pub fn cell_text(row: &[Value], idx: usize) -> String {
    row.get(idx)
        .map(format_cell)
        .unwrap_or_else(|| "-".to_string())
}

/// Gets a cell value as an integer, or 0 if not present.
pub fn cell_int(row: &[Value], idx: usize) -> i64 {
    match row.get(idx) {
        Some(Value::Int(n)) => *n,
        Some(Value::Text(s)) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

/// Gets a cell value as text, or a default if not present or empty.
pub fn cell_text_or(row: &[Value], idx: usize, default: &str) -> String {
    match row.get(idx) {
        Some(Value::Text(s)) if !s.is_empty() => s.clone(),
        _ => default.to_string(),
    }
}

/// Resolves the storage base directory for generated files.
///
/// Priority: `outputDirPath` arg > `config.storage_dir` > `~/.firebird-mcp/output/`.
/// Always returns `Some` (the default dir is a safe fallback).
pub fn storage_dir(cfg: &Config, args: &JsonValue) -> Option<String> {
    if let Some(output_dir) = arg_str(args, "outputDirPath").filter(|s| !s.is_empty()) {
        return Some(output_dir);
    }
    cfg.effective_storage_dir()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
}

/// Percent-encodes a single path segment for use in a URL query string.
///
/// Only the characters that are safe in a URL path/query are kept as-is; every
/// other byte is encoded as `%XX` (UTF-8). This is a minimal, dependency-free
/// encoder — enough for file paths, which are typically simple.
fn percent_encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Builds the HTTP download URL for a file that lives inside the storage
/// directory, when the transport exposes a `/files` endpoint.
///
/// Returns `None` when:
/// - `base_url` is `None` (stdio mode — no HTTP endpoint), or
/// - `abs_path` is not inside `storage_base` (e.g. an export written to a
///   caller-supplied path outside the storage dir, which `/files` cannot serve).
///
/// Otherwise returns `{base_url}/files?path=<url-encoded relative path>`.
pub fn file_download_url(
    base_url: Option<&str>,
    abs_path: &str,
    storage_base: &str,
) -> Option<String> {
    let base = base_url?;
    let abs = std::path::Path::new(abs_path);
    let rel = abs.strip_prefix(storage_base).ok()?;
    let encoded = rel
        .components()
        .map(|c| percent_encode_segment(c.as_os_str().to_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("/");
    if encoded.is_empty() {
        return None;
    }
    Some(format!("{base}/files?path={encoded}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_str() {
        let args = serde_json::json!({"name": "test", "count": 5});
        assert_eq!(arg_str(&args, "name"), Some("test".into()));
        assert_eq!(arg_str(&args, "missing"), None);
    }

    #[test]
    fn test_arg_u32() {
        let args = serde_json::json!({"n": 42, "s": "100", "b": true});
        assert_eq!(arg_u32(&args, "n"), Some(42));
        assert_eq!(arg_u32(&args, "s"), Some(100));
        assert_eq!(arg_u32(&args, "b"), None);
    }

    #[test]
    fn test_file_download_url_none_base() {
        assert_eq!(
            file_download_url(
                None,
                "/home/u/.firebird-mcp/output/db_pq/x.parquet",
                "/home/u/.firebird-mcp/output"
            ),
            None
        );
    }

    #[test]
    fn test_file_download_url_inside_storage() {
        let url = file_download_url(
            Some("http://10.0.0.5:8080"),
            "/home/u/.firebird-mcp/output/teste_parquet/20260101/data.parquet",
            "/home/u/.firebird-mcp/output",
        )
        .unwrap();
        assert_eq!(
            url,
            "http://10.0.0.5:8080/files?path=teste_parquet/20260101/data.parquet"
        );
    }

    #[test]
    fn test_file_download_url_encodes_spaces() {
        let url = file_download_url(
            Some("http://10.0.0.5:8080"),
            "/base/out/my file.csv",
            "/base",
        )
        .unwrap();
        assert_eq!(url, "http://10.0.0.5:8080/files?path=out/my%20file.csv");
    }

    #[test]
    fn test_plural_suffix() {
        assert_eq!(plural_suffix(1), "");
        assert_eq!(plural_suffix(0), "s");
        assert_eq!(plural_suffix(5), "s");
    }
}
