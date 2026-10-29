//! Stdio transport: JSON-RPC 2.0, line-delimited, over stdin/stdout.
//!
//! This is the standard MCP transport for IDEs and CLI tools. Each line on
//! stdin is a JSON-RPC request; each response is written to stdout as a
//! single JSON line.

use mcp_core::mlog;
use mcp_protocol::ProtocolCore;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

/// Runs the stdio transport: reads JSON-RPC lines from stdin.
///
/// Writes responses to stdout, one JSON line per response.
///
/// # Arguments
/// * `core` — the protocol core (shared with the SSE transport).
pub fn run_stdio(core: Arc<ProtocolCore>) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                mlog!("ignoring malformed JSON line: {e}");
                continue;
            }
        };

        if let Some(response) = core.handle_request(&request) {
            let mut out = response.to_string();
            out.push('\n');
            if writer.write_all(out.as_bytes()).is_err() {
                break;
            }
            let _ = writer.flush();
        }
    }
}
