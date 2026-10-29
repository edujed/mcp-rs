//! # mcp-protocol
//!
//! Transport-agnostic MCP protocol core.
//!
//! This crate encapsulates the JSON-RPC 2.0 request handling that is
//! independent of the underlying transport (stdio, HTTP/SSE, etc.). It
//! holds the shared, reloadable configuration and routes incoming
//! requests to the appropriate handler.
//!
//! The consuming crates (`mcp-transport` for stdio/SSE) are responsible
//! for reading requests from their respective transports and writing
//! responses back. This crate only transforms a request into a response.
//!
//! ## Architecture
//!
//! - [`ToolHandler`] — trait that each MCP server implements to register
//!   its tools.
//! - [`ProtocolCore`] — the JSON-RPC router. Holds the config and the
//!   tool handler, and routes `initialize`, `tools/list`, `tools/call`.
//! - [`events`] — universal POST_EVENT support (JSONL + Firebird event).
//! - [`common`] — shared helpers (arg parsing, client opening, Markdown).

use mcp_core::{Config, mlog};
use mcp_types::ToolResult;
use serde_json::{Value, json};
use std::sync::RwLock;

/// MCP protocol version supported by this server.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Trait that each MCP server implements to register its tools.
///
/// The platform provides the JSON-RPC routing, config management, and
/// event emission. The server provides the tool definitions and the
/// dispatch logic.
pub trait ToolHandler: Send + Sync {
    /// Server name reported in the `initialize` response.
    fn server_name(&self) -> &str;

    /// Server version reported in the `initialize` response.
    fn server_version(&self) -> &str;

    /// Build timestamp, generated at compile time.
    fn build_time(&self) -> &'static str;

    /// Returns the JSON schemas for `tools/list`.
    fn tool_definitions(&self) -> Value;

    /// Dispatches a tool call to the appropriate handler.
    ///
    /// `files_base_url` is `None` for stdio mode and `Some("http://host:port")`
    /// for SSE mode, so that file-generating tools can advertise a
    /// `download_url` in their responses.
    fn dispatch(
        &self,
        cfg: &Config,
        name: &str,
        args: &Value,
        files_base_url: Option<&str>,
    ) -> Result<ToolResult, String>;
}

/// Shared, reloadable configuration plus the protocol routing logic.
///
/// The config is loaded once at startup and can be refreshed by tool
/// handlers (e.g. `get_databases` re-reads the file from disk) or updated
/// in-memory (e.g. `create_database` after registering a new db).
///
/// `ProtocolCore` is designed to be shared across threads (e.g. one
/// instance per SSE client, or a single instance behind an async runtime),
/// so all mutable state is behind an `RwLock`.
pub struct ProtocolCore {
    config: RwLock<Option<Config>>,
    /// Base URL of the `/files` HTTP endpoint (e.g. `http://192.168.1.50:8080`),
    /// set only in SSE mode. When present, file-generating tools include a
    /// `download_url` in their structured response. `None` in stdio mode.
    files_base_url: Option<String>,
    handler: Box<dyn ToolHandler>,
}

/// Provides the JSON-RPC 2.0 request routing and shared config for both transports.
impl ProtocolCore {
    /// Creates a new protocol core with the given initial configuration.
    ///
    /// `files_base_url` is `None` for stdio mode (no HTTP endpoint) and
    /// `Some("http://host:port")` for SSE mode, so that file-generating tools
    /// can advertise a `download_url` in their responses.
    pub fn new(cfg: Config, files_base_url: Option<String>, handler: Box<dyn ToolHandler>) -> Self {
        Self {
            config: RwLock::new(Some(cfg)),
            files_base_url,
            handler,
        }
    }

    /// Returns the base URL of the `/files` endpoint, if any (SSE mode).
    pub fn files_base_url(&self) -> Option<&str> {
        self.files_base_url.as_deref()
    }

    /// Returns a read guard on the current configuration.
    ///
    /// Panics if the configuration has not been set (should not happen in
    /// normal operation, since `new` always provides one).
    pub fn config(&self) -> std::sync::RwLockReadGuard<'_, Option<Config>> {
        self.config.read().unwrap()
    }

    /// Replaces the current configuration.
    ///
    /// Used by tool handlers to register a new database in-memory after
    /// writing it to disk.
    pub fn set_config(&self, cfg: Config) {
        *self.config.write().unwrap() = Some(cfg);
    }

    /// Processes a single JSON-RPC 2.0 request and returns the response.
    ///
    /// Returns `None` for notifications (requests without an `id`), which
    /// per the MCP spec must not produce a response. The caller is
    /// responsible for silently acknowledging them.
    ///
    /// The request is expected to be a valid JSON-RPC 2.0 object. Malformed
    /// requests (missing `method`, unknown methods, etc.) are handled
    /// internally and produce an appropriate JSON-RPC error response.
    pub fn handle_request(&self, request: &Value) -> Option<Value> {
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Notifications (no `id`) must not produce a response. The MCP client
        // sends `notifications/initialized` right after `initialize`; per the
        // spec we acknowledge it silently. Other notifications are ignored.
        if request.get("id").is_none() {
            if method == "notifications/initialized" {
                mlog!("initialized");
            }
            return None;
        }

        let id = request.get("id").cloned();
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        let response: Result<Value, ()> = match method {
            "initialize" => {
                let result = json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": self.handler.server_name(),
                        "version": self.handler.server_version(),
                        "buildTime": self.handler.build_time()
                    }
                });
                Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            }
            "ping" => Ok(json!({ "jsonrpc": "2.0", "id": id, "result": json!({}) })),
            "tools/list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": self.handler.tool_definitions() }
            })),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                // Get a read guard on the config for this tool call
                let cfg_guard = self.config.read().unwrap();
                let cfg = cfg_guard.as_ref().unwrap();
                mlog!("tool call: {name}");
                let started = std::time::Instant::now();
                let result =
                    self.handler
                        .dispatch(cfg, name, &args, self.files_base_url.as_deref());
                let duration_ms = started.elapsed().as_millis();

                if let Err(ref e) = result {
                    mlog!("tool {name} ERROR: {e}");
                }

                // Universal POST_EVENT: fires for any tool that has a valid event param
                // and a resolvable database.
                let success = result.is_ok();
                let detail = result.as_ref().ok().and_then(|r| r.structured.as_ref());
                let error = result.as_ref().err().map(|s| s.as_str());
                events::emit_tool_event(cfg, name, &args, success, duration_ms, detail, error);

                match result {
                    Ok(ToolResult { text, structured }) => {
                        let mut result = json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": false
                        });
                        if let Some(s) = structured {
                            result["structuredContent"] = s;
                        }
                        Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                    }
                    Err(e) => {
                        let result = json!({
                            "content": [{ "type": "text", "text": e }],
                            "isError": true
                        });
                        Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                    }
                }
            }
            _ => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            })),
        };

        response.ok()
    }
}

/// Shared helpers for argument parsing, client creation, and Markdown output.
pub mod common;
/// Universal POST_EVENT support — emits events after every tool call.
pub mod events;
