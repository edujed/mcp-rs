# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial extraction of the shared MCP platform from firebird-5-mcp.
- `mcp-types` — shared value types (`Value`, `ColumnMeta`, `QueryResult`, `ToolResult`) + pure formatting helpers.
- `mcp-core` — Firebird access layer (`FirebirdClient`, `Config`, `mlog!`, paths, query execution with row-limit injection, scripts, batch inserts).
- `mcp-protocol` — transport-agnostic protocol core (`ToolHandler` trait, `ProtocolCore` JSON-RPC 2.0 router, universal `POST_EVENT`).
- `mcp-transport` — stdio and HTTP+SSE transports, bearer-token auth, file upload/download endpoints.
