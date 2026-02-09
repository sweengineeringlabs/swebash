/// Performance and AI-specific test assertions.
///
/// Provides latency percentile assertions (p95, p99), throughput checks,
/// eventual consistency testing, and AI error format validation.

use std::time::Duration;

use swebash_ai::api::error::AiError;

// ── Percentile Utilities ─────────────────────────────────────────────

/// Compute the `p`-th percentile of a sorted slice of durations.
///
/// `p` is expressed as a fraction in `[0.0, 1.0]` (e.g., 0.95 for p95).
///
/// # Panics
///
/// Panics if `samples` is empty or `p` is outside `[0.0, 1.0]`.
pub fn percentile(samples: &[Duration], p: f64) -> Duration {
    assert!(!samples.is_empty(), "percentile requires at least one sample");
    assert!(
        (0.0..=1.0).contains(&p),
        "percentile must be between 0.0 and 1.0, got {p}"
    );

    let mut sorted: Vec<Duration> = samples.to_vec();
    sorted.sort();

    let index = ((sorted.len() as f64 - 1.0) * p).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Assert that the p95 latency is within the given threshold.
///
/// # Panics
///
/// Panics if the p95 of `samples` exceeds `threshold`.
pub fn assert_latency_p95(samples: &[Duration], threshold: Duration) {
    let p95 = percentile(samples, 0.95);
    assert!(
        p95 <= threshold,
        "p95 latency {p95:?} exceeds threshold {threshold:?}"
    );
}

/// Assert that the p99 latency is within the given threshold.
///
/// # Panics
///
/// Panics if the p99 of `samples` exceeds `threshold`.
pub fn assert_latency_p99(samples: &[Duration], threshold: Duration) {
    let p99 = percentile(samples, 0.99);
    assert!(
        p99 <= threshold,
        "p99 latency {p99:?} exceeds threshold {threshold:?}"
    );
}

// ── Throughput ───────────────────────────────────────────────────────

/// Assert that throughput (operations per second) is above the given minimum.
///
/// # Parameters
///
/// - `ops_count`: total number of operations completed
/// - `elapsed`: wall-clock time for all operations
/// - `min_ops_per_sec`: minimum acceptable throughput
///
/// # Panics
///
/// Panics if measured throughput is below `min_ops_per_sec`.
pub fn assert_throughput_above(ops_count: u64, elapsed: Duration, min_ops_per_sec: f64) {
    let secs = elapsed.as_secs_f64();
    assert!(secs > 0.0, "elapsed duration must be positive");
    let throughput = ops_count as f64 / secs;
    assert!(
        throughput >= min_ops_per_sec,
        "throughput {throughput:.2} ops/s is below minimum {min_ops_per_sec:.2} ops/s"
    );
}

// ── Eventual Consistency ─────────────────────────────────────────────

/// Assert that a condition becomes true within a timeout period.
///
/// Polls the `check` closure repeatedly with `interval` between polls.
/// Passes as soon as `check` returns `true`.
///
/// # Panics
///
/// Panics if `check` never returns `true` within `timeout`.
pub async fn assert_eventually_consistent<F>(
    check: F,
    interval: Duration,
    timeout: Duration,
    message: &str,
) where
    F: Fn() -> bool,
{
    let start = tokio::time::Instant::now();
    loop {
        if check() {
            return;
        }
        if start.elapsed() >= timeout {
            panic!(
                "assert_eventually_consistent failed after {timeout:?}: {message}"
            );
        }
        tokio::time::sleep(interval).await;
    }
}

// ── AI-Specific Assertions ───────────────────────────────────────────

/// Assert that an `AiError` Display representation starts with the expected prefix.
///
/// Useful for validating that error messages follow the expected format
/// (e.g., "AI provider error: ...").
///
/// # Panics
///
/// Panics if the error's Display output does not start with `expected_prefix`.
pub fn assert_ai_error_format(err: &AiError, expected_prefix: &str) {
    let msg = err.to_string();
    assert!(
        msg.starts_with(expected_prefix),
        "Expected error to start with '{expected_prefix}', got: '{msg}'"
    );
}

/// Assert that an error is the kind we expect when the provider cannot be
/// initialised (missing key, bad config, unreachable service, etc.).
///
/// Accepts `AiError::NotConfigured` or `AiError::Provider`.
///
/// # Panics
///
/// Panics if the error is any other variant.
pub fn assert_setup_error(err: &AiError) {
    match err {
        AiError::NotConfigured(_) | AiError::Provider(_) => {}
        other => panic!(
            "Expected NotConfigured or Provider for missing configuration, got: {:?}",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_single_sample() {
        let samples = vec![Duration::from_millis(100)];
        assert_eq!(percentile(&samples, 0.95), Duration::from_millis(100));
    }

    #[test]
    fn percentile_multiple_samples() {
        let samples: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();
        let p95 = percentile(&samples, 0.95);
        assert!(p95 >= Duration::from_millis(95));
        assert!(p95 <= Duration::from_millis(100));
    }

    #[test]
    fn percentile_p0_returns_min() {
        let samples = vec![
            Duration::from_millis(10),
            Duration::from_millis(50),
            Duration::from_millis(100),
        ];
        assert_eq!(percentile(&samples, 0.0), Duration::from_millis(10));
    }

    #[test]
    fn percentile_p100_returns_max() {
        let samples = vec![
            Duration::from_millis(10),
            Duration::from_millis(50),
            Duration::from_millis(100),
        ];
        assert_eq!(percentile(&samples, 1.0), Duration::from_millis(100));
    }

    #[test]
    #[should_panic(expected = "percentile requires at least one sample")]
    fn percentile_panics_on_empty() {
        percentile(&[], 0.5);
    }

    #[test]
    #[should_panic(expected = "percentile must be between")]
    fn percentile_panics_on_invalid_p() {
        percentile(&[Duration::from_millis(1)], 1.5);
    }

    #[test]
    fn assert_latency_p95_passes() {
        let samples: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();
        assert_latency_p95(&samples, Duration::from_millis(200));
    }

    #[test]
    #[should_panic(expected = "p95 latency")]
    fn assert_latency_p95_fails() {
        let samples: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();
        assert_latency_p95(&samples, Duration::from_millis(50));
    }

    #[test]
    fn assert_latency_p99_passes() {
        let samples: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();
        assert_latency_p99(&samples, Duration::from_millis(200));
    }

    #[test]
    #[should_panic(expected = "p99 latency")]
    fn assert_latency_p99_fails() {
        let samples: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();
        assert_latency_p99(&samples, Duration::from_millis(50));
    }

    #[test]
    fn assert_throughput_above_passes() {
        assert_throughput_above(1000, Duration::from_secs(1), 500.0);
    }

    #[test]
    #[should_panic(expected = "throughput")]
    fn assert_throughput_above_fails() {
        assert_throughput_above(10, Duration::from_secs(1), 500.0);
    }

    #[tokio::test]
    async fn assert_eventually_consistent_passes_immediately() {
        assert_eventually_consistent(
            || true,
            Duration::from_millis(10),
            Duration::from_millis(100),
            "should pass",
        )
        .await;
    }

    #[tokio::test]
    async fn assert_eventually_consistent_passes_after_delay() {
        let start = std::time::Instant::now();
        assert_eventually_consistent(
            move || start.elapsed() >= Duration::from_millis(30),
            Duration::from_millis(10),
            Duration::from_secs(1),
            "should converge",
        )
        .await;
    }

    #[test]
    fn assert_ai_error_format_passes() {
        let err = AiError::Provider("something broke".into());
        assert_ai_error_format(&err, "AI provider error:");
    }

    #[test]
    #[should_panic(expected = "Expected error to start with")]
    fn assert_ai_error_format_fails() {
        let err = AiError::Timeout;
        assert_ai_error_format(&err, "AI provider error:");
    }

    #[test]
    fn assert_setup_error_accepts_not_configured() {
        let err = AiError::NotConfigured("missing key".into());
        assert_setup_error(&err);
    }

    #[test]
    fn assert_setup_error_accepts_provider() {
        let err = AiError::Provider("connection refused".into());
        assert_setup_error(&err);
    }

    #[test]
    #[should_panic(expected = "Expected NotConfigured or Provider")]
    fn assert_setup_error_rejects_other_variants() {
        let err = AiError::Timeout;
        assert_setup_error(&err);
    }
}
