/// Exponential backoff retry utilities.
///
/// Provides both synchronous and asynchronous retry functions with
/// configurable max attempts, initial delay, and backoff multiplier.

use std::time::Duration;

use crate::error::TestError;

/// Retry a synchronous closure with exponential backoff.
///
/// Calls `f` up to `max_attempts` times. If `f` returns `Err`, waits
/// for a geometrically increasing delay before retrying.
///
/// # Parameters
///
/// - `max_attempts`: maximum number of attempts (must be >= 1)
/// - `initial_delay`: delay before the first retry
/// - `f`: closure to retry; returns `Ok(T)` on success or `Err(String)` on failure
///
/// # Returns
///
/// `Ok(T)` if `f` succeeds within the attempt budget, or `Err(TestError::Mock)`
/// with the last failure message.
pub fn retry_with_backoff<T, F>(
    max_attempts: u32,
    initial_delay: Duration,
    f: F,
) -> Result<T, TestError>
where
    F: Fn() -> Result<T, String>,
{
    assert!(max_attempts >= 1, "max_attempts must be at least 1");
    let mut delay = initial_delay;
    let mut last_err = String::new();

    for attempt in 1..=max_attempts {
        match f() {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = e;
                if attempt < max_attempts {
                    std::thread::sleep(delay);
                    delay *= 2;
                }
            }
        }
    }

    Err(TestError::Mock(format!(
        "all {max_attempts} attempts failed; last error: {last_err}"
    )))
}

/// Retry an asynchronous closure with exponential backoff.
///
/// Calls `f` up to `max_attempts` times. If `f` returns `Err`, waits
/// for a geometrically increasing delay before retrying.
///
/// # Parameters
///
/// - `max_attempts`: maximum number of attempts (must be >= 1)
/// - `initial_delay`: delay before the first retry
/// - `f`: async closure to retry
///
/// # Returns
///
/// `Ok(T)` if `f` succeeds within the attempt budget, or `Err(TestError::Mock)`
/// with the last failure message.
pub async fn retry_with_backoff_async<T, F, Fut>(
    max_attempts: u32,
    initial_delay: Duration,
    f: F,
) -> Result<T, TestError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    assert!(max_attempts >= 1, "max_attempts must be at least 1");
    let mut delay = initial_delay;
    let mut last_err = String::new();

    for attempt in 1..=max_attempts {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = e;
                if attempt < max_attempts {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    Err(TestError::Mock(format!(
        "all {max_attempts} attempts failed; last error: {last_err}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn retry_succeeds_first_attempt() {
        let result = retry_with_backoff(3, Duration::from_millis(1), || Ok::<_, String>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn retry_succeeds_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(3, Duration::from_millis(1), move || {
            let n = counter_clone.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Err(format!("attempt {n} failed"))
            } else {
                Ok(n)
            }
        });

        assert_eq!(result.unwrap(), 2);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn retry_fails_after_all_attempts() {
        let result = retry_with_backoff(2, Duration::from_millis(1), || {
            Err::<(), _>("always fails".into())
        });

        match result {
            Err(TestError::Mock(msg)) => {
                assert!(msg.contains("all 2 attempts failed"));
                assert!(msg.contains("always fails"));
            }
            other => panic!("Expected Mock error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn retry_async_succeeds_first_attempt() {
        let result =
            retry_with_backoff_async(3, Duration::from_millis(1), || async { Ok::<_, String>(42) })
                .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retry_async_succeeds_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff_async(3, Duration::from_millis(1), move || {
            let c = counter_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(format!("attempt {n} failed"))
                } else {
                    Ok(n)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 2);
    }

    #[tokio::test]
    async fn retry_async_fails_after_all_attempts() {
        let result = retry_with_backoff_async(2, Duration::from_millis(1), || async {
            Err::<(), _>("async fail".into())
        })
        .await;

        match result {
            Err(TestError::Mock(msg)) => {
                assert!(msg.contains("all 2 attempts failed"));
                assert!(msg.contains("async fail"));
            }
            other => panic!("Expected Mock error, got: {other:?}"),
        }
    }
}
