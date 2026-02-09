/// Test framework error types.

use std::time::Duration;

/// Errors produced by the swebash-test framework.
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    /// Mock setup or invocation failure.
    #[error("mock error: {0}")]
    Mock(String),

    /// Fixture creation or cleanup failure.
    #[error("fixture error: {0}")]
    Fixture(String),

    /// Assertion failure with context.
    #[error("assertion error: {0}")]
    Assertion(String),

    /// Security scanner failure.
    #[error("security scanner error: {0}")]
    SecurityScanner(String),

    /// Test naming convention violation.
    #[error("naming violation: {0}")]
    NamingViolation(String),

    /// Operation timed out.
    #[error("timeout after {0:?}")]
    Timeout(Duration),

    /// Streaming test failure.
    #[error("stream error: {0}")]
    Stream(String),

    /// I/O error (from temp dirs, file writes, etc.).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_mock() {
        let err = TestError::Mock("client not configured".into());
        assert_eq!(err.to_string(), "mock error: client not configured");
    }

    #[test]
    fn test_error_display_fixture() {
        let err = TestError::Fixture("dir creation failed".into());
        assert_eq!(err.to_string(), "fixture error: dir creation failed");
    }

    #[test]
    fn test_error_display_assertion() {
        let err = TestError::Assertion("p99 exceeded threshold".into());
        assert_eq!(err.to_string(), "assertion error: p99 exceeded threshold");
    }

    #[test]
    fn test_error_display_timeout() {
        let err = TestError::Timeout(Duration::from_secs(5));
        assert_eq!(err.to_string(), "timeout after 5s");
    }

    #[test]
    fn test_error_display_stream() {
        let err = TestError::Stream("unexpected event after Done".into());
        assert_eq!(
            err.to_string(),
            "stream error: unexpected event after Done"
        );
    }

    #[test]
    fn test_error_display_naming_violation() {
        let err = TestError::NamingViolation("missing stress_test_ prefix".into());
        assert_eq!(
            err.to_string(),
            "naming violation: missing stress_test_ prefix"
        );
    }

    #[test]
    fn test_error_display_security_scanner() {
        let err = TestError::SecurityScanner("scanner init failed".into());
        assert_eq!(err.to_string(), "security scanner error: scanner init failed");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = TestError::from(io_err);
        assert!(err.to_string().contains("file missing"));
    }
}
