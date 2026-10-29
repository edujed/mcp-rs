# mcp-transport — Crate Summary (for AI agents)

> **Purpose:** the transport layer of the MCP platform. Provides the two
> supported transports — **stdio** (line-delimited JSON-RPC over
> stdin/stdout, the default for IDEs/CLI) and **HTTP+SSE** (spec
> `2024-11-05`, for web UIs and non-stdio clients like llama.cpp
> `--ui-mcp-proxy`, CODEX, ClaudeCode) — plus bearer-token auth and the
> file upload/download endpoints. Each transport is a thin layer over
> `mcp_protocol::ProtocolCore`; the consuming binary does CLI parsing,
> config loading, and transport selection.

## Files

| File | Lines | Role |
|---|---|---|
| `src/lib.rs` | 25 | Module declarations + re-exports (`check_auth`, `AuthConfig`, `build_router`, `run_sse`, `SseState`, `run_stdio`). |
| `src/auth.rs` | 162 | Bearer-token auth (disabled by default), constant-time comparison. |
| `src/sse.rs` | 355 | HTTP+SSE transport: `GET /sse`, `POST /messages`, CORS, session management. |
| `src/files.rs` | 356 | `GET /files?path=<rel>` (download) + `POST /files` (multipart upload, 500 MB limit), path-traversal protection. |
| `src/stdio.rs` | 49 | Stdio transport: JSON-RPC over stdin/stdout. |

## `src/auth.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct AuthConfig` | 24–53 | `token: Option<String>` — `None` disables auth. `disabled()`, `with_token()`, `is_enabled()`. |
| `fn check_auth` | 62–87 | Validates `Authorization: Bearer <token>` (accepts `bearer` lowercase too). Constant-time comparison to mitigate timing attacks. |

## `src/sse.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct Session` | 43–60 | One SSE connection: `broadcast::Sender<Value>` (64 slots) + `last_active`. |
| `struct SseState` | 66–71 | `Arc<ProtocolCore>` + session map + `Arc<AuthConfig>`. |
| `fn build_router` | 88–115 | Axum router: `GET /sse`, `POST /messages`, CORS preflight on both. |
| `fn handle_sse` | 153–204 | Opens a session, yields the `endpoint` event (`/messages?sessionId=<uuid>`), then bridges the broadcast channel to `message` events. Keep-alive ping every 15s. |
| `fn handle_message` | 208–292 | Parses the session ID + JSON-RPC body, looks up the session, runs `core.handle_request()` in `spawn_blocking`, sends the response through the channel. Returns 202 Accepted (the response arrives via the SSE stream). |
| `fn spawn_session_cleanup` | 297–323 | Reaps sessions inactive longer than the timeout (checked every 60s). |
| `fn run_sse` | 333–355 | Binds the listener, merges the SSE router with the files router, spawns cleanup. |

## `src/files.rs`

| Item | Lines | Summary |
|---|---|---|
| `const MAX_UPLOAD_BYTES` | 28 | 500 MB. |
| `fn build_files_router` | 45–65 | `GET /files` + `POST /files` + CORS preflight. |
| `fn resolve_storage_dir` | 100–104 | From the live config: `config.storage_dir` > default output dir. |
| `fn resolve_safe_path` | 110–168 | Path-traversal protection: rejects absolute paths and `..` components; canonicalizes and verifies containment in the base dir. For uploads, canonicalizes the deepest existing ancestor and re-joins the remainder. |
| `fn handle_download` | 171–219 | Streams a file with `Content-Disposition: attachment`. |
| `fn handle_upload` | 227–322 | Multipart: `file` (binary) + `target` (relative path). Creates parent dirs. |

## `src/stdio.rs`

| Item | Lines | Summary |
|---|---|---|
| `fn run_stdio` | 18–49 | Reads JSON-RPC lines from stdin, writes one JSON line per response to stdout. Malformed lines are logged and skipped. |

## Conventions

- **CORS is open (`*`)** on all SSE endpoints, including preflight (204 with
  `Access-Control-Allow-Origin: *`) — required for browser clients on
  different origins.
- **Auth is disabled by default** — the consuming binary enables it via
  `--token` / `--token-file` CLI flags.
- **SSE sessions are UUID-keyed** broadcast channels (64 slots); lagged
  messages are logged and skipped (the channel is a fire-and-forget
  delivery mechanism).
- **The storage directory is resolved from the live config** (the
  `RwLock<Option<Config>>` in `ProtocolCore`), so config reloads take effect
  without a restart.
