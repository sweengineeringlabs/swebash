//! DevOps tools for package management and file downloads.
//!
//! This module provides specialized tools for DevOps operations:
//! - `PackageManagerTool`: Cross-platform package management (apt/yum/dnf/brew/choco)
//! - `DownloadTool`: HTTP/HTTPS file download with checksum verification
//!
//! Error handling follows a structured approach with:
//! - Categorized errors (Validation, Network, Timeout, etc.)
//! - Actionable suggestions for users
//! - Automatic retry detection for transient failures

mod download;
mod error;
mod package_manager;

pub use download::DownloadTool;
pub use error::{DownloadError, ErrorCategory, IntoToolError, PackageManagerError};
pub use package_manager::PackageManagerTool;
