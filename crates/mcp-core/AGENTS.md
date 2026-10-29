# mcp-core — Crate Summary (for AI agents)

> **Purpose:** the Firebird access layer of the MCP platform. Provides the
> connection wrapper, configuration loading, daily file logging, path
> resolution, and all query execution (SELECT with row-limit injection,
> DML/DDL, scripts with `SET TERM`, batch inserts). Ported from
> `firebird-5-mcp/crates/mcp-core` with Firebird-specific storage/codegen
> code removed.

## Files

| File | Lines | Role |
|---|---|---|
| `src/lib.rs` | 30 | Module declarations + re-exports (`FirebirdClient`, `IsolationPreset`, `Config`, `DatabaseConfig`, `ServerConfig`). |
| `src/client.rs` | 237 | `FirebirdClient` (one connection per database, pure-Rust driver), `IsolationPreset` (4 presets), transaction management. |
| `src/config.rs` | 316 | `Config` / `ServerConfig` / `DatabaseConfig` — load/save/validate `config.json`, database registration, `effective_storage_dir()`. |
| `src/log.rs` | 81 | Daily file logging (`{log_dir}/yyyy-mm-dd.log`), `mlog!` macro. |
| `src/paths.rs` | 116 | Path resolution from a single base dir (`~/.firebird-mcp/` or `$FIREBIRD_MCP_HOME`). |
| `src/query/mod.rs` | 292 | `query` / `query_with_limit` / `query_with_param` / `query_unbounded`, `exec`, `execute`, `post_event`, NUMERIC/DECIMAL detection, `sql_type_name`. |
| `src/query/batch.rs` | 164 | `run_script` (multi-statement transaction), `insert_batch` (prepared statement), `count`, `sample`. |
| `src/query/row_limit.rs` | 153 | `FIRST {max} SKIP {skip}` injection (`accepts_row_limit`, `inject_row_limit`), `DEFAULT_ROWS=100`, `MAX_ROWS=1000`. |
| `src/query/split.rs` | 228 | SQL statement splitting: `SET TERM` blocks, `BEGIN`/`END`, strings, comments. |

## `src/client.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct FirebirdClient` | 15–20 | Owns a `SimpleConnection` + `ServerConfig` + `db_path`. One connection per database file; the MCP layer opens a client per tool call (no pooling). |
| `enum IsolationPreset` | 56–74 | `ReadCommited` (default), `Snapshot`, `Consistency`, `ReadOnly`. The driver (v0.29) exposes only these three isolation levels. |
| `impl IsolationPreset` | 77–130 | `parse()` (string form for the MCP `isolation` param), `label()`, `to_conf(lock_wait)`. |
| `impl FirebirdClient` | 133–237 | `new()` (connects with read-only metadata txn), `create_database()` (valid page sizes 1K–32K), `db_path()`, `second_connection()` (for explain-plan), `execute_immediate()` (SET TERM scripts), `begin_rw_transaction()`, `begin_transaction(preset, lock_wait)`. |

## `src/config.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct DatabaseConfig` | 28–34 | `name`, `description`, `path`. |
| `struct ServerConfig` | 36–53 | `host`, `port`, `user`, `password`, `allowed_paths`, `databases`, `default_database`, legacy `isql_path` (ignored). |
| `struct Config` | 55–64 | `server` + optional `storage_dir` override. |
| `impl Config` | 66–257 | `load()` (lookup: `FIREBIRD_CONFIG` env → `./config.json` → `~/.firebird-mcp/config.json`), `load_or_create()` (never overwrites an existing file), `default_empty()`, `save()` / `save_to()`, `register_database()` (upsert, first db becomes default), `validate()`, `default_database()`, `reload()`, `effective_storage_dir()`. |
| `impl ServerConfig` | 259–294 | `find_database()` (case-insensitive), `database_names()`, `is_path_allowed()`. |
| `fn config_path` | 302–311 | Resolves the config file path (env → cwd → base dir). |

## `src/query/`

| Item | Lines | Summary |
|---|---|---|
| `fn query` | 31–33 | SELECT with default row limit (100). |
| `fn query_with_limit` | 36–52 | SELECT with explicit `max_rows`/`skip`; injects `FIRST`/`SKIP` when absent. |
| `fn query_with_param` | 55–65 | SELECT with a single string parameter. |
| `fn query_unbounded` | 71–78 | No row-limit injection — for bulk data movement (file export). Never use for LLM-facing queries. |
| `fn rows_to_result` | 81–140 | Driver rows → `QueryResult`; NUMERIC/DECIMAL detection (raw_type 580 + decimal text) and precision/scale inference from data. |
| `fn exec` | 181–199 | DML/DDL in an explicit read-write transaction; returns affected row count. |
| `fn post_event` | 206–221 | `POST_EVENT '<name>'` in its own transaction (notifies `SET EVENT` listeners on commit). |
| `fn execute` | 225–241 | Tries SELECT first, falls back to DML/DDL. |
| `fn run_script` | 25–54 (batch.rs) | Multi-statement script in one transaction (split via `split_statements`); rollback on failure. |
| `fn insert_batch` | 58–109 (batch.rs) | Prepared-statement batch insert in one transaction. |
| `fn count` / `fn sample` | 133–164 (batch.rs) | `COUNT(*)` with optional WHERE; `FIRST {limit}` sample. |
| `fn inject_row_limit` | 31–75 (row_limit.rs) | Inserts `FIRST {max} SKIP {skip}` after `SELECT` (or after the CTE definition for `WITH`); respects existing limit clauses. |
| `fn split_statements` | 13–19 (split.rs) | Splits a SQL batch into statements, handling `SET TERM`, strings, comments, `BEGIN`/`END`. |

## Conventions

- **Firebird-specific naming:** `FIREBIRD_CONFIG` env var, `~/.firebird-mcp/`
  base dir, `FIREBIRD_MCP_HOME` override. Generalization is a Phase 3 concern.
- **`mlog!` is exported via `#[macro_export]`** — consumers use
  `mcp_core::mlog!`. Logging never crashes the server (silent drop on failure).
- **Row limits are mandatory for LLM-facing queries:** `query()` caps at
  `DEFAULT_ROWS=100`, hard cap `MAX_ROWS=1000`. Use `query_unbounded()` only
  for bulk export.
- **NUMERIC/DECIMAL:** the driver does not propagate precision/scale — they
  are inferred from the data (max integer digits + max fractional digits,
  precision capped at 18).
