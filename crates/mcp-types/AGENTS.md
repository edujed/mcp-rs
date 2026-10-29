# mcp-types — Crate Summary (for AI agents)

> **Purpose:** shared value types for the MCP platform. This crate holds the
> data shapes consumed by more than one crate: value types, result
> containers, tool result envelopes, and pure formatting helpers.
> **No IO, no async, no driver code** — it is safe to depend on from anywhere.

## Files

| File | Lines | Role |
|---|---|---|
| `src/lib.rs` | 334 | Everything: `Value`, `ColumnMeta`, `QueryResult`, `ToolResult`, `format_cell`, `is_valid_identifier`, `value_to_json`, `base64_encode` |

## `src/lib.rs`

| Item | Lines | Summary |
|---|---|---|
| `enum Value` | 20–32 | Single column value: `Null`, `Bool`, `Int(i64)`, `Float(f64)`, `Text`, `Date`, `Time`, `Timestamp`, `Binary`. Derives `PartialEq` (needed for `assert_eq!` in tests). |
| `impl Value` | 82–101 | `as_i64()` (parses text, coerces bools), `as_text()` (text or integer). |
| `struct ColumnMeta` | 34–56 | Column name, `type_name`, `is_numeric` flag (NUMERIC/DECIMAL detection — the vendor patch renders scaled integers as exact decimal text but `raw_type` stays 580), inferred `precision`/`scale`. |
| `struct QueryResult` | 58–80 | `columns` + `rows`; `get(row, col)` (Null on OOB), `col_index(name)` (case-insensitive). |
| `struct ToolResult` | 112–135 | Envelope returned by every MCP tool call: `text` (always) + `structured` (optional JSON). Constructors `text()` / `with_structured()`. |
| `fn is_valid_identifier` | 143–150 | SQL identifier validation (letters/digits/underscore, must start with a letter). |
| `fn format_cell` | 158–176 | Markdown cell rendering: NULLs → "NULL", newlines flattened, pipes escaped, floats rendered as integers when whole. |
| `fn value_to_json` | 209–230 | JSON serialization for `structuredContent`: NUMERIC/DECIMAL stays a string (precision), `Binary` → base64. |
| `fn base64_encode` | 233–254 | Minimal base64 encoder (avoids a dependency). |

## Conventions

- **`Value` needs `PartialEq`** — tests use `assert_eq!` on values.
- **NUMERIC/DECIMAL fidelity:** the vendor patch in
  `firebird-5-mcp/vendor/rsfbclient-rust` renders scaled integers as exact
  decimal text. `ColumnMeta.is_numeric` + `precision`/`scale` let the export
  path map to `Decimal128` instead of `Int64`.
- **No dependencies beyond serde/serde_json/chrono.** Keep it that way —
  this crate is the leaf of the dependency graph.
