# mcp-protocol — Crate Summary (for AI agents)

> **Purpose:** transport-agnostic MCP protocol core. Encapsulates the
> JSON-RPC 2.0 request handling that is independent of the underlying
> transport (stdio, HTTP/SSE). Holds the shared, reloadable configuration
> and routes incoming requests to the server's `ToolHandler`. The consuming
> transports (`mcp-transport`) read requests and write responses; this crate
> only transforms a request into a response.

## Files

| File | Lines | Role |
|---|---|---|
| `src/lib.rs` | 222 | `ToolHandler` trait, `ProtocolCore` (JSON-RPC router), `PROTOCOL_VERSION`. |
| `src/common.rs` | 246 | Arg helpers (`arg_str`, `arg_bool`, `arg_u32`), `open_client`, `result_to_text` (Markdown), `storage_dir`, `file_download_url`, `percent_encode_segment`. |
| `src/events.rs` | 271 | Universal POST_EVENT: `emit_tool_event`, `build_event`, `resolve_database`, JSONL append. |

## `src/lib.rs`

| Item | Lines | Summary |
|---|---|---|
| `const PROTOCOL_VERSION` | 29 | `"2024-11-05"`. |
| `trait ToolHandler` | 36–61 | **The extension point.** `server_name()`, `server_version()`, `build_time()`, `tool_definitions() -> Value`, `dispatch(cfg, name, args, files_base_url) -> Result<ToolResult, String>`. |
| `struct ProtocolCore` | 72–79 | `RwLock<Option<Config>>` + `files_base_url` (SSE only) + `Box<dyn ToolHandler>`. |
| `impl ProtocolCore` | 82–217 | `new(cfg, files_base_url, handler)`, `files_base_url()`, `config()` (read guard), `set_config()`, `handle_request(&Value) -> Option<Value>` (synchronous; `None` for notifications). |

### `handle_request` routing

| Method | Behavior |
|---|---|
| `initialize` | Returns `protocolVersion`, `capabilities.tools`, `serverInfo` (name/version/buildTime from the handler). |
| `ping` | Empty result. |
| `tools/list` | Returns `handler.tool_definitions()`. |
| `tools/call` | Extracts `name`/`arguments`, takes a read guard on the config, calls `handler.dispatch()`, measures duration, fires `events::emit_tool_event()` (universal POST_EVENT), wraps the result in the MCP `content`/`isError`/`structuredContent` envelope. |
| (no `id`) | Notification — silently acknowledged, no response. |
| (unknown) | JSON-RPC error `-32601 Method not found`. |

## `src/common.rs`

| Item | Lines | Summary |
|---|---|---|
| `fn arg_str` / `arg_bool` / `arg_u32` | 13–33 | JSON arg extraction. `arg_u32` accepts both JSON numbers and numeric strings (LLMs send both). |
| `fn open_client` | 36–47 | Opens a `FirebirdClient` for the `database` arg (or the default). |
| `fn result_to_text` | 50–91 | Renders a `QueryResult` as a Markdown table (columns with types, `format_cell` per value). |
| `fn plural_suffix` | 94–100 | `""` for 1, `"s"` otherwise. |
| `fn cell_text` / `cell_int` / `cell_text_or` | 103–124 | Row cell accessors with defaults. |
| `fn storage_dir` | 130–136 | `outputDirPath` arg > `config.storage_dir` > default output dir. |
| `fn file_download_url` | 165–182 | `{base_url}/files?path=<encoded rel path>` when the file is inside the storage dir and a base URL exists (SSE mode). |

## `src/events.rs`

| Item | Lines | Summary |
|---|---|---|
| `fn events_file` | 26–28 | `{base}/events.jsonl`. |
| `fn valid_event_name` | 32–36 | Trims and rejects empty event names. |
| `fn build_event` | 43–71 | Structured payload: `event`, `tool`, `success`, `database`, `timestamp`, `duration_ms`, `detail`, `error`. |
| `fn resolve_database` | 76–86 | `database` arg or configured default; validates against the database list. |
| `fn emit_tool_event` | 97–161 | **Never fails the tool call.** 1) Appends a JSON line to `events.jsonl` (always). 2) When a valid database resolves, opens a **fresh** connection and executes `POST_EVENT '<name>'` in its own transaction (the handler's connection may be in an unknown state after a failure). |

## Conventions

- **`handle_request` is synchronous** — the SSE transport wraps it in
  `tokio::task::spawn_blocking` (database I/O can be slow).
- **`files_base_url` is `None` in stdio mode** — file-generating tools omit
  `download_url` in that case.
- **Event emission is best-effort:** all errors are logged via `mlog!` and
  dropped; the tool result is returned unchanged.
