//! # mcp-core
//!
//! MCP platform core: Firebird database access, configuration, logging,
//! and path resolution.
//!
//! This crate is the reusable foundation for all MCP servers in the
//! orchestra ecosystem. It provides:
//!
//! - [`config`] — loading of `config.json`
//! - [`client`] — a connection wrapper per database
//! - [`query`] — SELECT results, DML/DDL, scripts, batch inserts
//! - [`log`] — file-based daily logging
//! - [`paths`] — resolved filesystem paths for config, storage, and events
//!
//! The MCP protocol layer (`mcp-protocol`) and transport layer
//! (`mcp-transport`) sit on top of this crate.

/// Connection wrapper per database (pure-Rust driver).
pub mod client;
/// Loading of `config.json`.
pub mod config;
/// File-based daily logging.
pub mod log;
/// Resolved filesystem paths for config, storage, and events.
pub mod paths;
/// SELECT results, DML/DDL, scripts, batch inserts.
pub mod query;

pub use client::{FirebirdClient, IsolationPreset};
pub use config::{Config, DatabaseConfig, ServerConfig};
