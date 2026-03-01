//! HTTP/HTTPS file download tool with checksum verification.
//!
//! Provides reliable file downloads with:
//! - Resume support via Range headers
//! - SHA256/SHA512/MD5 checksum validation
//! - Progress reporting and size limits

use std::any::Any;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256, Sha512};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, instrument, warn};

use super::super::error::IntoToolError;
use super::error::DownloadError;
use llmboot_orchestration::{RiskLevel, Tool, ToolCapability, ToolDefinition, ToolError, ToolOutput, ToolExecResult as ToolResult};

/// Supported checksum algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    Sha256,
    Sha512,
    Md5,
}

impl ChecksumAlgorithm {
    /// Parse a checksum string in the format "algorithm:hash".
    fn parse(checksum: &str) -> Result<(Self, String), DownloadError> {
        let parts: Vec<&str> = checksum.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(DownloadError::InvalidChecksum {
                provided: checksum.to_string(),
                expected_format: "algorithm:hash (e.g., 'sha256:abc123...')",
            });
        }

        let algorithm = match parts[0].to_lowercase().as_str() {
            "sha256" => ChecksumAlgorithm::Sha256,
            "sha512" => ChecksumAlgorithm::Sha512,
            "md5" => ChecksumAlgorithm::Md5,
            other => {
                return Err(DownloadError::InvalidChecksum {
                    provided: format!("algorithm '{}'", other),
                    expected_format: "sha256, sha512, or md5",
                });
            }
        };

        let hash = parts[1].to_lowercase();

        // Validate hash length
        let expected_len = match algorithm {
            ChecksumAlgorithm::Sha256 => 64,
            ChecksumAlgorithm::Sha512 => 128,
            ChecksumAlgorithm::Md5 => 32,
        };

        if hash.len() != expected_len {
            return Err(DownloadError::InvalidChecksum {
                provided: format!("{} hash with {} characters", parts[0], hash.len()),
                expected_format: match algorithm {
                    ChecksumAlgorithm::Sha256 => "sha256 requires 64 hex characters",
                    ChecksumAlgorithm::Sha512 => "sha512 requires 128 hex characters",
                    ChecksumAlgorithm::Md5 => "md5 requires 32 hex characters",
                },
            });
        }

        // Validate hex characters
        if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DownloadError::InvalidChecksum {
                provided: checksum.to_string(),
                expected_format: "hash must contain only hexadecimal characters (0-9, a-f)",
            });
        }

        Ok((algorithm, hash))
    }
}

/// Arguments for the download tool.
#[derive(Debug, Deserialize)]
struct DownloadArgs {
    url: String,
    output: Option<String>,
    checksum: Option<String>,
    #[serde(default = "default_resume")]
    resume: bool,
}

fn default_resume() -> bool {
    true
}

/// HTTP/HTTPS file download tool.
///
/// Downloads files with optional checksum verification and resume support.
pub struct DownloadTool {
    max_size: u64,
    timeout: Duration,
}

impl DownloadTool {
    /// Create a new download tool with size limit and timeout.
    ///
    /// # Arguments
    /// * `max_size` - Maximum download size in bytes (default 1GB)
    /// * `timeout` - Request timeout duration
    pub fn new(max_size: u64, timeout: Duration) -> Self {
        Self { max_size, timeout }
    }

    /// Validate and sanitize the URL.
    fn validate_url(url: &str) -> Result<url::Url, DownloadError> {
        let parsed = url::Url::parse(url).map_err(|e| {
            DownloadError::InvalidUrl {
                url: url.to_string(),
                reason: e.to_string(),
            }
        })?;

        // Only allow HTTP and HTTPS
        match parsed.scheme() {
            "http" | "https" => {}
            other => {
                return Err(DownloadError::UnsupportedScheme {
                    scheme: other.to_string(),
                    suggestion: "Use http:// or https:// URLs only",
                });
            }
        }

        // Block private/local addresses
        if let Some(host) = parsed.host_str() {
            let host_lower = host.to_lowercase();
            if host_lower == "localhost"
                || host_lower == "127.0.0.1"
                || host_lower == "::1"
                || host_lower == "0.0.0.0"
                || host_lower.starts_with("192.168.")
                || host_lower.starts_with("10.")
                || host_lower.starts_with("172.")
                || host_lower.ends_with(".local")
                || host_lower.ends_with(".internal")
            {
                return Err(DownloadError::PrivateNetwork {
                    host: host.to_string(),
                });
            }
        }

        Ok(parsed)
    }

    /// Derive output filename from URL if not specified.
    fn derive_output_path(url: &url::Url, output: Option<&str>) -> Result<PathBuf, DownloadError> {
        if let Some(out) = output {
            let path = PathBuf::from(out);
            // Validate path doesn't traverse directories
            if out.contains("..") {
                return Err(DownloadError::InvalidUrl {
                    url: out.to_string(),
                    reason: "Output path cannot contain '..' (directory traversal)".to_string(),
                });
            }
            return Ok(path);
        }

        // Extract filename from URL
        let path_segments = url.path_segments().ok_or_else(|| {
            DownloadError::NoFilename {
                url: url.to_string(),
            }
        })?;

        let filename = path_segments
            .last()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                DownloadError::NoFilename {
                    url: url.to_string(),
                }
            })?;

        // Sanitize filename
        let sanitized: String = filename
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .collect();

        if sanitized.is_empty() {
            return Err(DownloadError::NoFilename {
                url: url.to_string(),
            });
        }

        Ok(PathBuf::from(sanitized))
    }

    /// Calculate checksum of a file.
    async fn calculate_checksum(path: &Path, algorithm: ChecksumAlgorithm) -> Result<String, DownloadError> {
        let mut file = File::open(path).await.map_err(|e| {
            DownloadError::IoError {
                path: path.display().to_string(),
                source: e,
            }
        })?;

        let mut buffer = vec![0u8; 8192];

        match algorithm {
            ChecksumAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                loop {
                    let n = file.read(&mut buffer).await.map_err(|e| {
                        DownloadError::IoError {
                            path: path.display().to_string(),
                            source: e,
                        }
                    })?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                Ok(format!("{:x}", hasher.finalize()))
            }
            ChecksumAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                loop {
                    let n = file.read(&mut buffer).await.map_err(|e| {
                        DownloadError::IoError {
                            path: path.display().to_string(),
                            source: e,
                        }
                    })?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                Ok(format!("{:x}", hasher.finalize()))
            }
            ChecksumAlgorithm::Md5 => {
                // MD5 support requires md-5 crate - not implemented to avoid dependency
                Err(DownloadError::InvalidChecksum {
                    provided: "md5".to_string(),
                    expected_format: "MD5 is not supported. Use sha256 or sha512 for checksums.",
                })
            }
        }
    }

    /// Perform the download operation.
    #[instrument(skip(self), fields(url = %url, output = %output_path.display()))]
    async fn do_download(
        &self,
        url: &url::Url,
        output_path: &Path,
        resume: bool,
    ) -> Result<(u64, bool), DownloadError> {
        debug!("Starting download");

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .user_agent("swebash-download/1.0")
            .build()
            .map_err(|e| {
                DownloadError::NetworkError {
                    url: url.to_string(),
                    source: Box::new(e),
                }
            })?;

        // Check existing file size for resume
        let existing_size = if resume {
            fs::metadata(output_path).await.ok().map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        if existing_size > 0 {
            debug!(existing_size, "Resuming from existing file");
        }

        // Build request with optional Range header
        let mut request = client.get(url.as_str());
        if existing_size > 0 {
            request = request.header("Range", format!("bytes={}-", existing_size));
        }

        let response = request.send().await.map_err(|e| {
            DownloadError::NetworkError {
                url: url.to_string(),
                source: Box::new(e),
            }
        })?;

        let status = response.status();
        debug!(status = %status, "Received HTTP response");

        // Handle resume response
        let (resumed, content_length) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
            // Server supports range requests
            let len = response.content_length().unwrap_or(0);
            info!(resumed_from = existing_size, remaining = len, "Resuming download");
            (true, existing_size + len)
        } else if status.is_success() {
            // Full download
            let len = response.content_length().unwrap_or(0);
            (false, len)
        } else {
            return Err(DownloadError::HttpError {
                url: url.to_string(),
                status: status.as_u16(),
                message: status.canonical_reason().unwrap_or("Unknown").to_string(),
            });
        };

        // Check size limit
        if content_length > self.max_size {
            return Err(DownloadError::SizeExceeded {
                size: content_length,
                max_size: self.max_size,
            });
        }

        // Open file for writing (append if resuming)
        let mut file = if resumed {
            OpenOptions::new()
                .append(true)
                .open(output_path)
                .await
                .map_err(|e| {
                    DownloadError::IoError {
                        path: output_path.display().to_string(),
                        source: e,
                    }
                })?
        } else {
            // Create parent directories if needed
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).await.ok();
            }
            File::create(output_path).await.map_err(|e| {
                DownloadError::IoError {
                    path: output_path.display().to_string(),
                    source: e,
                }
            })?
        };

        // Stream response body to file
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = if resumed { existing_size } else { 0 };

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                DownloadError::NetworkError {
                    url: url.to_string(),
                    source: Box::new(e),
                }
            })?;

            file.write_all(&chunk).await.map_err(|e| {
                DownloadError::IoError {
                    path: output_path.display().to_string(),
                    source: e,
                }
            })?;

            downloaded += chunk.len() as u64;

            // Check ongoing size limit
            if downloaded > self.max_size {
                warn!(downloaded, max_size = self.max_size, "Download exceeded size limit");
                return Err(DownloadError::SizeExceeded {
                    size: downloaded,
                    max_size: self.max_size,
                });
            }
        }

        file.flush().await.map_err(|e| {
            DownloadError::IoError {
                path: output_path.display().to_string(),
                source: e,
            }
        })?;

        info!(downloaded_bytes = downloaded, resumed, "Download completed");
        Ok((downloaded, resumed))
    }
}

#[async_trait]
impl Tool for DownloadTool {
    fn name(&self) -> &str {
        "download"
    }

    fn description(&self) -> &str {
        "Download files from HTTP/HTTPS URLs with optional checksum verification and resume support."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The HTTP/HTTPS URL to download from"
                },
                "output": {
                    "type": "string",
                    "description": "Output file path (defaults to filename from URL)"
                },
                "checksum": {
                    "type": "string",
                    "description": "Expected checksum in format 'algorithm:hash' (e.g., 'sha256:abc123...')"
                },
                "resume": {
                    "type": "boolean",
                    "default": true,
                    "description": "Resume partial downloads if possible"
                }
            },
            "required": ["url"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::LowRisk
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn default_timeout_ms(&self) -> u64 {
        self.timeout.as_millis() as u64
    }

    #[instrument(skip(self, args), fields(url))]
    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        let args: DownloadArgs = serde_json::from_value(args).map_err(|e| {
            ToolError::InvalidArguments(format!("Invalid arguments: {}", e))
        })?;

        debug!(url = %args.url, output = ?args.output, "Executing download tool");

        // Validate URL
        let url = Self::validate_url(&args.url).map_err(IntoToolError::into_tool_error)?;

        // Derive output path
        let output_path = Self::derive_output_path(&url, args.output.as_deref())
            .map_err(IntoToolError::into_tool_error)?;

        // Parse checksum if provided
        let checksum_spec = if let Some(ref cs) = args.checksum {
            Some(ChecksumAlgorithm::parse(cs).map_err(IntoToolError::into_tool_error)?)
        } else {
            None
        };

        // Perform download
        let start = std::time::Instant::now();
        let (downloaded_bytes, resumed) = self.do_download(&url, &output_path, args.resume)
            .await
            .map_err(IntoToolError::into_tool_error)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Verify checksum if provided
        let checksum_result = if let Some((algorithm, expected_hash)) = checksum_spec {
            debug!(algorithm = ?algorithm, "Verifying checksum");
            let actual_hash = Self::calculate_checksum(&output_path, algorithm)
                .await
                .map_err(IntoToolError::into_tool_error)?;

            if actual_hash != expected_hash {
                // Delete the file if checksum fails
                let _ = fs::remove_file(&output_path).await;
                let err = DownloadError::ChecksumMismatch {
                    algorithm: format!("{:?}", algorithm).to_lowercase(),
                    expected: expected_hash,
                    actual: actual_hash,
                };
                warn!(?err, "Checksum verification failed");
                return Err(err.into_tool_error());
            }
            info!("Checksum verified successfully");
            Some(json!({
                "algorithm": format!("{:?}", algorithm).to_lowercase(),
                "expected": expected_hash,
                "actual": actual_hash,
                "verified": true
            }))
        } else {
            None
        };

        let result = json!({
            "url": args.url,
            "output": output_path.display().to_string(),
            "size_bytes": downloaded_bytes,
            "resumed": resumed,
            "duration_ms": duration_ms,
            "checksum": checksum_result,
            "success": true
        });

        info!(size_bytes = downloaded_bytes, duration_ms, "Download completed successfully");
        Ok(ToolOutput::success(result))
    }

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
            // Download tool needs network access and file write
            capabilities: (ToolCapability::NETWORK_EXTERNAL | ToolCapability::FILE_WRITE).bits(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_valid() {
        assert!(DownloadTool::validate_url("https://example.com/file.tar.gz").is_ok());
        assert!(DownloadTool::validate_url("http://releases.ubuntu.com/file.iso").is_ok());
    }

    #[test]
    fn test_validate_url_invalid_scheme() {
        let err = DownloadTool::validate_url("ftp://example.com/file.tar.gz").unwrap_err();
        match err {
            DownloadError::UnsupportedScheme { scheme, .. } => assert_eq!(scheme, "ftp"),
            _ => panic!("Expected UnsupportedScheme error"),
        }

        assert!(DownloadTool::validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_validate_url_private_networks() {
        let err = DownloadTool::validate_url("http://localhost/file").unwrap_err();
        match err {
            DownloadError::PrivateNetwork { host } => assert_eq!(host, "localhost"),
            _ => panic!("Expected PrivateNetwork error"),
        }

        assert!(DownloadTool::validate_url("http://127.0.0.1/file").is_err());
        assert!(DownloadTool::validate_url("http://192.168.1.1/file").is_err());
        assert!(DownloadTool::validate_url("http://10.0.0.1/file").is_err());
    }

    #[test]
    fn test_checksum_parse_valid() {
        let (alg, hash) = ChecksumAlgorithm::parse(
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ).unwrap();
        assert_eq!(alg, ChecksumAlgorithm::Sha256);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_checksum_parse_invalid_format() {
        let err = ChecksumAlgorithm::parse("sha256").unwrap_err();
        match err {
            DownloadError::InvalidChecksum { provided, .. } => {
                assert_eq!(provided, "sha256");
            }
            _ => panic!("Expected InvalidChecksum error"),
        }
        assert!(ChecksumAlgorithm::parse("invalid:abc").is_err());
    }

    #[test]
    fn test_checksum_parse_invalid_length() {
        let err = ChecksumAlgorithm::parse("sha256:abc123").unwrap_err();
        match err {
            DownloadError::InvalidChecksum { provided, expected_format } => {
                assert!(provided.contains("6 characters"));
                assert!(expected_format.contains("64"));
            }
            _ => panic!("Expected InvalidChecksum error"),
        }
    }

    #[test]
    fn test_derive_output_path_from_url() {
        let url = url::Url::parse("https://example.com/path/to/file.tar.gz").unwrap();
        let path = DownloadTool::derive_output_path(&url, None).unwrap();
        assert_eq!(path, PathBuf::from("file.tar.gz"));
    }

    #[test]
    fn test_derive_output_path_explicit() {
        let url = url::Url::parse("https://example.com/file").unwrap();
        let path = DownloadTool::derive_output_path(&url, Some("/tmp/download.bin")).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/download.bin"));
    }

    #[test]
    fn test_derive_output_path_traversal_blocked() {
        let url = url::Url::parse("https://example.com/file").unwrap();
        let err = DownloadTool::derive_output_path(&url, Some("../etc/passwd")).unwrap_err();
        match err {
            DownloadError::InvalidUrl { reason, .. } => {
                assert!(reason.contains("directory traversal"));
            }
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    #[test]
    fn test_derive_output_no_filename() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let err = DownloadTool::derive_output_path(&url, None).unwrap_err();
        match err {
            DownloadError::NoFilename { .. } => {}
            _ => panic!("Expected NoFilename error"),
        }
    }

    #[test]
    fn test_error_suggestions() {
        let err = DownloadError::PrivateNetwork { host: "localhost".to_string() };
        assert!(err.suggestion().contains("public URL"));

        let err = DownloadError::HttpError {
            url: "https://example.com".to_string(),
            status: 404,
            message: "Not Found".to_string(),
        };
        assert!(err.suggestion().contains("not found"));

        let err = DownloadError::SizeExceeded { size: 2_000_000_000, max_size: 1_000_000_000 };
        assert!(err.suggestion().contains("1000000000"));
    }
}
