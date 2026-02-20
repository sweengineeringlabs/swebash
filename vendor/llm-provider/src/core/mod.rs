//! LLM Core - Service implementation

pub mod context;
mod metrics;
mod ratelimit;
mod resilience;
mod service;

pub use context::{ContextConfig, ContextConfigBuilder, ContextValidator, ValidationResult};
pub use metrics::{global_metrics, init_global_metrics, LlmMetrics, MetricsTimer};
pub use ratelimit::{
    acquire_rate_limit, global_limiter, try_acquire_rate_limit, RateLimitConfig, RateLimiter,
};
pub use resilience::{with_retry, with_retry_config};
pub use service::DefaultLlmService;
