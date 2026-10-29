# mcp-rs — Project Summary (for AI agents)

> **Purpose:** token-efficient map of the project root. Read this first to
> locate a crate, script, or example before opening the full source.
>
> **What this project is:** the shared MCP (Model Context Protocol) platform
> in Rust. A workspace of four library crates that any MCP server can build
> on: value types, Firebird database access, the JSON-RPC 2.0 protocol core,
> and the stdio/SSE transports. Each concrete MCP server (firebird-5-mcp,
> orchestra, future sqlserver-mcp) implements the `ToolHandler` trait and
> gets protocol routing, transports, auth, file endpoints, and event
> emission for free.

## Subdirectories

| Directory | AGENTS.md | Role |
|---|---|---|
| `crates/mcp-types/` | [`crates/mcp-types/AGENTS.md`](crates/mcp-types/AGENTS.md) | Shared value types (`Value`, `ColumnMeta`, `QueryResult`, `ToolResult`) + pure formatting helpers. No IO, no async, no driver code. |
| `crates/mcp-core/` | [`crates/mcp-core/AGENTS.md`](crates/mcp-core/AGENTS.md) | Firebird access layer — `FirebirdClient`, `Config`, `mlog!`, path resolution, query execution (SELECT/DML/DDL, scripts, batch inserts, row-limit injection, SQL splitting). |
| `crates/mcp-protocol/` | [`crates/mcp-protocol/AGENTS.md`](crates/mcp-protocol/AGENTS.md) | Transport-agnostic protocol core — `ToolHandler` trait, `ProtocolCore` (JSON-RPC router), arg helpers, universal POST_EVENT. |
| `crates/mcp-transport/` | [`crates/mcp-transport/AGENTS.md`](crates/mcp-transport/AGENTS.md) | Transports — stdio (line-delimited JSON-RPC), HTTP+SSE (spec `2024-11-05`), bearer-token auth, file upload/download endpoints. |

## `scripts/`

| File | Role |
|---|---|
| `publish_new_tag.sh` | Release helper — commit, tag, push to origin (GitHub Actions publishes the release) |

**Usage:**
```bash
./scripts/publish_new_tag.sh v0.1.0 "feat: initial release"
```

## `.github/workflows/`

| File | Role |
|---|---|
| `ci.yml` | On push/PR to main — check, fmt, clippy (`-D warnings`), test. Fetches the vendored `rsfbclient-rust` from `edujed/firebird-5-mcp` and re-points the `[patch.crates-io]` path. |
| `release.yml` | On `v*` tags — verifies the build/tests and publishes the GitHub Release (library workspace: no binaries to build). |

## Conventions

- **Four-crate workspace:** `mcp-transport` depends on `mcp-protocol` which
  depends on `mcp-core` which depends on `mcp-types`. See each crate's
  `AGENTS.md` for function-level detail. (The `orchestra-parser` crate lives
  in the orchestra repo — it is not part of this workspace.)
- **`ToolHandler` is the extension point:** each MCP server implements
  `mcp_protocol::ToolHandler` (server name/version, tool definitions,
  `dispatch()`). `ProtocolCore::new(cfg, files_base_url, handler)` wires it
  in. The platform owns everything else (routing, config, events).
- **Two transports, one protocol core:** `run_stdio(Arc<ProtocolCore>)` for
  stdio mode; `run_sse(core, host, port, auth, session_timeout_min)` for
  HTTP+SSE. SSE sessions are UUID-keyed broadcast channels, reaped after the
  configured inactivity timeout. Optional bearer-token auth, disabled by
  default.
- **Universal POST_EVENT:** every tool call accepts optional
  `on_success_event` / `on_fail_event` string params. After the handler
  completes, `ProtocolCore` calls `events::emit_tool_event()`, which appends
  a JSON line to `{base}/events.jsonl` and — when a valid database is
  resolved — opens a fresh connection and executes `POST_EVENT '<name>'` in
  its own transaction. Emission never fails the tool call.
- **Firebird-specific naming is intentional (for now):** `mcp-core` uses
  `FIREBIRD_CONFIG` / `FIREBIRD_MCP_HOME` env vars and `~/.firebird-mcp/`
  base dir, inherited from firebird-5-mcp. Generalizing these names is a
  Phase 3 concern (when the platform is published and consumed by
  non-Firebird MCPs).
- **`[patch.crates-io]` is critical:** the workspace root `Cargo.toml`
  patches `rsfbclient-rust` to the vendored copy at
  `/home/edujed/github/firebird-5-mcp/vendor/rsfbclient-rust` (NUMERIC/DECIMAL
  precision fix). Without it, `cargo check` fails. The path is absolute —
  it will break if that repo moves.
- **`span-locations` is a `proc-macro2` feature, not a `syn` feature.**
  Adding it to `syn`'s features causes a version resolution error.
- **Edition 2024** across the workspace; `rustfmt --edition 2024` for
  formatting.

## Build & test

```bash
cargo check          # all 4 crates, 0 warnings
cargo test           # 42 tests (19 mcp-core, 10 mcp-protocol, 5 mcp-transport, 8 mcp-types)
cargo clippy --all-targets   # 0 warnings (2 known `result_large_err` in mcp-transport, axum Response)
cargo fmt --check
```

No binary is produced — this is a library workspace. Consuming projects add
the crates as path dependencies (see `README.md` for a usage example).

## Release

```bash
./scripts/publish_new_tag.sh v0.1.0 "feat: initial release"
```

The tag triggers `.github/workflows/release.yml`, which verifies the build
and publishes the GitHub Release. The workspace version lives in
`[workspace.package].version` in the root `Cargo.toml` — bump it before
tagging.
