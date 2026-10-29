//! # mcp-transport
//!
//! Transport layer for the MCP platform.
//!
//! This crate provides the two supported transports:
//!
//! - **stdio** — JSON-RPC 2.0, line-delimited, over stdin/stdout (default).
//! - **SSE** — HTTP+SSE (spec `2024-11-05`), for web UIs and non-stdio clients.
//!
//! Each transport is a thin layer over [`mcp_protocol::ProtocolCore`].
//! The consuming binary is responsible for CLI parsing, config loading,
//! and choosing which transport to run.

/// Bearer-token authentication for the SSE transport.
pub mod auth;
/// File upload/download endpoints for the SSE transport.
pub mod files;
/// HTTP+SSE transport (spec `2024-11-05`).
pub mod sse;
/// Stdio transport: JSON-RPC over stdin/stdout.
pub mod stdio;

pub use auth::{AuthConfig, check_auth};
pub use sse::{SseState, build_router, run_sse};
pub use stdio::run_stdio;
