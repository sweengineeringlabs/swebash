//! Rate limiting for LLM API calls
//!
//! Provides token bucket rate limiting to prevent exceeding provider rate limits.

use crate::api::{LlmError, LlmResult};
use rustboot_ratelimit::TokenBucket;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Default requests per minute for unknown providers
const DEFAULT_RPM: usize = 60;

/// Default refill interval (1 second)
const DEFAULT_REFILL_INTERVAL_MS: u64 = 1000;

/// Rate limit configuration for a provider
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute
    pub requests_per_minute: usize,
    /// Tokens per minute (for token-based limiting)
    pub tokens_per_minute: Option<usize>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: DEFAULT_RPM,
            tokens_per_minute: None,
        }
    }
}

impl RateLimitConfig {
    /// Create config for OpenAI
    pub fn openai() -> Self {
        Self {
            requests_per_minute: 500, // Tier 1 default
            tokens_per_minute: Some(30_000),
        }
    }

    /// Create config for Anthropic
    pub fn anthropic() -> Self {
        Self {
            requests_per_minute: 50, // Build tier default
            tokens_per_minute: Some(40_000),
        }
    }

    /// Create config for Gemini
    pub fn gemini() -> Self {
        Self {
            requests_per_minute: 60,
            tokens_per_minute: Some(60_000),
        }
    }
}

/// Rate limiter for LLM providers
pub struct RateLimiter {
    buckets: RwLock<HashMap<String, Arc<TokenBucket>>>,
    configs: RwLock<HashMap<String, RateLimitConfig>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
        }
    }

    /// Configure rate limits for a provider
    pub async fn configure(&self, provider: &str, config: RateLimitConfig) {
        let mut configs = self.configs.write().await;
        configs.insert(provider.to_string(), config.clone());

        // Create the bucket
        let bucket = create_bucket(&config);
        let mut buckets = self.buckets.write().await;
        buckets.insert(provider.to_string(), Arc::new(bucket));

        debug!(
            provider = provider,
            rpm = config.requests_per_minute,
            "Configured rate limiter"
        );
    }

    /// Acquire permission to make a request (non-blocking)
    pub async fn try_acquire(&self, provider: &str) -> LlmResult<()> {
        let buckets = self.buckets.read().await;

        if let Some(bucket) = buckets.get(provider) {
            bucket.try_acquire().await.map_err(|_| LlmError::RateLimited {
                retry_after_ms: Some(1000),
            })
        } else {
            // No limiter configured, allow request
            Ok(())
        }
    }

    /// Acquire permission, waiting if necessary
    pub async fn acquire(&self, provider: &str) -> LlmResult<()> {
        // Ensure bucket exists for provider
        {
            let buckets = self.buckets.read().await;
            if !buckets.contains_key(provider) {
                drop(buckets);
                // Create default bucket
                let config = default_config_for_provider(provider);
                self.configure(provider, config).await;
            }
        }

        let buckets = self.buckets.read().await;
        if let Some(bucket) = buckets.get(provider) {
            bucket.acquire().await.map_err(|_| LlmError::RateLimited {
                retry_after_ms: Some(1000),
            })
        } else {
            Ok(())
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

fn create_bucket(config: &RateLimitConfig) -> TokenBucket {
    // Convert RPM to tokens per second
    let capacity = config.requests_per_minute / 60 + 1; // Allow burst
    let refill_rate = capacity.max(1);
    let refill_interval = Duration::from_millis(DEFAULT_REFILL_INTERVAL_MS);

    TokenBucket::new(capacity, refill_rate, refill_interval)
}

fn default_config_for_provider(provider: &str) -> RateLimitConfig {
    match provider {
        "openai" => RateLimitConfig::openai(),
        "anthropic" => RateLimitConfig::anthropic(),
        "gemini" => RateLimitConfig::gemini(),
        _ => RateLimitConfig::default(),
    }
}

/// Global rate limiter instance
static GLOBAL_LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();

/// Get the global rate limiter
pub fn global_limiter() -> &'static RateLimiter {
    GLOBAL_LIMITER.get_or_init(RateLimiter::new)
}

/// Acquire rate limit permission for a provider
pub async fn acquire_rate_limit(provider: &str) -> LlmResult<()> {
    global_limiter().acquire(provider).await
}

/// Try to acquire rate limit permission (non-blocking)
pub async fn try_acquire_rate_limit(provider: &str) -> LlmResult<()> {
    global_limiter().try_acquire(provider).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::new();
        limiter
            .configure("test", RateLimitConfig::default())
            .await;

        // Should allow requests
        assert!(limiter.try_acquire("test").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_unknown_provider() {
        let limiter = RateLimiter::new();

        // Unknown provider should still work (no limiting)
        assert!(limiter.try_acquire("unknown").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_default_configs() {
        assert_eq!(RateLimitConfig::openai().requests_per_minute, 500);
        assert_eq!(RateLimitConfig::anthropic().requests_per_minute, 50);
        assert_eq!(RateLimitConfig::gemini().requests_per_minute, 60);
    }

    #[tokio::test]
    async fn test_acquire_creates_bucket() {
        let limiter = RateLimiter::new();

        // acquire should create bucket for unknown provider
        assert!(limiter.acquire("new_provider").await.is_ok());

        // Now try_acquire should also work
        assert!(limiter.try_acquire("new_provider").await.is_ok());
    }
}
