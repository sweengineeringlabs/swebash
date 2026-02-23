//! Web tools for HTTP operations and content fetching.
//!
//! This module provides tools for web interactions:
//! - HTTP client operations (GET, POST, etc.)
//! - Content fetching with size limits
//! - Response parsing and validation
//!
//! Error handling follows the shared pattern with:
//! - Categorized errors (Validation, Network, Timeout, etc.)
//! - Actionable suggestions for users
//! - Automatic retry detection for transient failures

mod error;

pub use error::HttpError;
