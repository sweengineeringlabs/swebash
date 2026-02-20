//! Resilience patterns for LLM operations
//!
//! Provides retry with exponential backoff for transient failures.

use crate::api::LlmResult;
use crate::config::ProviderConfig;
use rustboot_resilience::ExponentialBackoff;
use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Default initial delay for retries (100ms)
const DEFAULT_INITIAL_DELAY_MS: u64 = 100;

/// Default maximum delay for retries (30 seconds)
const DEFAULT_MAX_DELAY_MS: u64 = 30_000;

/// Execute an LLM operation with retry on transient failures
///
/// Uses exponential backoff with the retry count from ProviderConfig.
/// Only retries errors where `LlmError::is_retryable()` returns true.
///
/// # Example
/// ```ignore
/// let result = with_retry(&config, || async {
///     provider.complete(&request).await
/// }).await;
/// ```
pub async fn with_retry<F, Fut, T>(config: &ProviderConfig, operation: F) -> LlmResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = LlmResult<T>>,
{
    with_retry_config(
        config.max_retries as usize,
        DEFAULT_INITIAL_DELAY_MS,
        DEFAULT_MAX_DELAY_MS,
        operation,
    )
    .await
}

/// Execute an LLM operation with custom retry configuration
pub async fn with_retry_config<F, Fut, T>(
    max_retries: usize,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    mut operation: F,
) -> LlmResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = LlmResult<T>>,
{
    if max_retries == 0 {
        return operation().await;
    }

    let backoff = ExponentialBackoff::new(
        Duration::from_millis(initial_delay_ms),
        Duration::from_millis(max_delay_ms),
        2.0,
    );

    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;

                // Check if we should retry
                if !e.is_retryable() {
                    debug!(
                        error = %e,
                        "Non-retryable error, failing immediately"
                    );
                    return Err(e);
                }

                if attempts > max_retries {
                    warn!(
                        attempts = attempts,
                        max_retries = max_retries,
                        error = %e,
                        "Max retries exceeded"
                    );
                    return Err(e);
                }

                // Calculate delay, respecting rate limit hints
                let delay = e.retry_after().unwrap_or_else(|| backoff.next_delay(attempts));

                debug!(
                    attempt = attempts,
                    max_retries = max_retries,
                    delay_ms = delay.as_millis(),
                    error = %e,
                    "Retrying after transient error"
                );

                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::LlmError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn test_config(max_retries: u32) -> ProviderConfig {
        ProviderConfig {
            max_retries,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_success_first_attempt() {
        let config = test_config(3);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = Arc::clone(&calls);

        let result = with_retry(&config, || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, LlmError>(42)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_on_network_error() {
        let config = test_config(3);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = Arc::clone(&calls);

        let result = with_retry(&config, || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let count = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 3 {
                    Err(LlmError::NetworkError("connection reset".to_string()))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_no_retry_on_auth_error() {
        let config = test_config(3);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = Arc::clone(&calls);

        let result = with_retry(&config, || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(LlmError::AuthenticationFailed("invalid key".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        // Should only call once - auth errors are not retryable
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let config = test_config(2);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = Arc::clone(&calls);

        let result = with_retry(&config, || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(LlmError::NetworkError("always fails".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        // Initial attempt + 2 retries = 3 calls
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_zero_retries() {
        let config = test_config(0);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = Arc::clone(&calls);

        let result = with_retry(&config, || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(LlmError::NetworkError("fails".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        // With 0 retries, should only call once
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
