# mcp-rs

Shared [MCP](https://modelcontextprotocol.io/) (Model Context Protocol) platform in Rust. A workspace of four reusable library crates that any MCP server can build on: value types, Firebird database access, the JSON-RPC 2.0 protocol core, and the stdio/SSE transports.

Each concrete MCP server (e.g. [firebird-5-mcp](https://github.com/edujed/firebird-5-mcp), [orchestra](https://github.com/edujed/orchestra), a future sqlserver-mcp) implements a single `ToolHandler` trait and gets protocol routing, both transports, auth, file endpoints, and event emission for free.

## Crates

| Crate | Role |
|---|---|
| [`mcp-types`](crates/mcp-types) | Shared value types (`Value`, `ColumnMeta`, `QueryResult`, `ToolResult`) + pure formatting helpers. No IO, no async, no driver code. |
| [`mcp-core`](crates/mcp-core) | Firebird access layer — `FirebirdClient`, `Config`, `mlog!`, path resolution, query execution (SELECT with row-limit injection, DML/DDL, scripts with `SET TERM`, batch inserts). |
| [`mcp-protocol`](crates/mcp-protocol) | Transport-agnostic protocol core — `ToolHandler` trait, `ProtocolCore` (JSON-RPC 2.0 router), arg helpers, universal `POST_EVENT`. |
| [`mcp-transport`](crates/mcp-transport) | Transports — stdio (line-delimited JSON-RPC) and HTTP+SSE (spec `2024-11-05`), bearer-token auth, file upload/download endpoints. |

Dependency chain: `mcp-transport` → `mcp-protocol` → `mcp-core` → `mcp-types`.

## Features

- **Two transports, one protocol core.** `run_stdio(Arc<ProtocolCore>)` for IDEs/CLI; `run_sse(core, host, port, auth, session_timeout_min)` for web UIs and non-stdio clients (llama.cpp `--ui-mcp-proxy`, CODEX, ClaudeCode). SSE sessions are UUID-keyed broadcast channels, reaped after the configured inactivity timeout.
- **Optional bearer-token auth** for the SSE transport (disabled by default, constant-time comparison).
- **File endpoints (SSE only).** `GET /files?path=<rel>` (download, path-traversal protected) and `POST /files` (multipart upload, 500 MB limit), with CORS for browser clients.
- **Universal `POST_EVENT`.** Every tool call accepts optional `on_success_event` / `on_fail_event` string parameters. After the handler completes, the platform appends a JSON line to `{base}/events.jsonl` and — when a valid database is resolved — opens a fresh connection and executes `POST_EVENT '<name>'` in its own transaction (notifying Firebird `SET EVENT` listeners on commit). Emission never fails the tool call.
- **LLM-safe queries.** `mcp-core` injects a `FIRST {max} SKIP {skip}` clause into bare `SELECT`s (default 100 rows, hard cap 1000) so a `SELECT *` cannot flood the context window.
- **Exact NUMERIC/DECIMAL fidelity.** The workspace patches `rsfbclient-rust` to a vendored copy that keeps scaled integers as exact decimal text instead of coercing to `f64` (see [Vendored driver](#vendored-driver)).

## Usage

Add the crates as path dependencies in your MCP server's `Cargo.toml`:

```toml
[dependencies]
mcp-types = { path = "path/to/mcp-rs/crates/mcp-types" }
mcp-core = { path = "path/to/mcp-rs/crates/mcp-core" }
mcp-protocol = { path = "path/to/mcp-rs/crates/mcp-protocol" }
mcp-transport = { path = "path/to/mcp-rs/crates/mcp-transport" }
```

(When the crates are published to crates.io, replace the path dependencies with version requirements.)

Implement `ToolHandler` and wire it into the protocol core:

```rust
use mcp_core::Config;
use mcp_protocol::{ProtocolCore, ToolHandler};
use mcp_types::ToolResult;
use serde_json::{json, Value};
use std::sync::Arc;

struct MyHandler;

impl ToolHandler for MyHandler {
    fn server_name(&self) -> &str { "my-mcp" }
    fn server_version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn build_time(&self) -> &'static str { env!("CARGO_BUILD_TIME") }

    fn tool_definitions(&self) -> Value {
        json!([{
            "name": "hello",
            "description": "Say hello",
            "inputSchema": { "type": "object", "properties": {} }
        }])
    }

    fn dispatch(&self, _cfg: &Config, name: &str, _args: &Value, _files_base_url: Option<&str>)
        -> Result<ToolResult, String>
    {
        match name {
            "hello" => Ok(ToolResult::text("Hello!")),
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

fn main() {
    let cfg = Config::load_or_create().expect("config");
    let core = Arc::new(ProtocolCore::new(cfg, None, Box::new(MyHandler)));
    mcp_transport::run_stdio(core);
}
```

For SSE mode, call `mcp_transport::run_sse(core, "0.0.0.0", 8080, Arc::new(AuthConfig::disabled()), 30)` inside a tokio runtime instead.

## Configuration

`mcp-core` loads `config.json` (lookup order: `FIREBIRD_CONFIG` env var → `./config.json` → `~/.firebird-mcp/config.json`):

```json
{
  "server": {
    "host": "localhost",
    "port": 3050,
    "user": "SYSDBA",
    "password": "masterkey",
    "allowed_paths": ["/database"],
    "databases": [
      { "name": "teste", "description": "Test database", "path": "/database/teste.fdb" }
    ],
    "default_database": "teste"
  }
}
```

Filesystem layout (base dir `~/.firebird-mcp/`, override with `FIREBIRD_MCP_HOME`):

```
~/.firebird-mcp/
├── config.json           — server configuration
├── output/               — generated files (storage dir)
├── logs/                 — daily log files (yyyy-mm-dd.log)
└── events.jsonl          — tool event log
```

> **Note:** the Firebird-specific naming (`FIREBIRD_CONFIG`, `FIREBIRD_MCP_HOME`, `~/.firebird-mcp/`) is inherited from firebird-5-mcp and intentional for now. Generalizing these names for non-Firebird consumers is a planned follow-up.

## Vendored driver

The workspace root `Cargo.toml` patches `rsfbclient-rust` to a vendored copy (in the [firebird-5-mcp](https://github.com/edujed/firebird-5-mcp) repo, `vendor/rsfbclient-rust/`) that stops coercing NUMERIC/DECIMAL columns to `f64` — the upstream driver loses exactness on `NUMERIC(18,4)`, the standard money type. The patch keeps the scaled integer + scale in `xsqlda.rs` and renders the exact decimal as text at fetch time.

The patch path is absolute — if you consume this workspace from a different machine, adjust `[patch.crates-io]` in the root `Cargo.toml` (or remove it and accept `f64` coercion).

## Build & test

```bash
cargo check          # all 4 crates, 0 warnings
cargo test           # 42 tests (19 mcp-core, 10 mcp-protocol, 5 mcp-transport, 8 mcp-types)
cargo clippy --all-targets
cargo fmt --check
```

No binary is produced — this is a library workspace.

## Roadmap

- **Phase 1 (current):** extract the shared platform from firebird-5-mcp; publish as reusable crates.
- **Phase 2:** generalize the Firebird-specific naming (`FIREBIRD_CONFIG`, `~/.firebird-mcp/`) so non-Firebird MCPs (e.g. sqlserver-mcp) can use the platform without awkward env vars.
- **Phase 3:** publish to crates.io; add a `Streamable HTTP` transport (spec `2025-03-26`) alongside the legacy SSE transport.

## License

[MIT](LICENSE)
