/// Reusable test framework for the swebash workspace.
///
/// Provides shared mock objects, RAII fixtures, streaming assertions,
/// security scanners, naming conventions, contract verifiers, and
/// observability capture following the rustboot-test pattern adapted
/// for the AI/LLM/shell domain.
///
/// # Architecture
///
/// Single-Crate Flat SEA (infrastructure utility):
///
/// ```text
/// lib.rs        — module declarations + prelude
/// error.rs      — TestError enum
/// mock.rs       — AI mock infrastructure
/// fixture.rs    — RAII temp directories, scoped fixtures, env var guards
/// naming.rs     — test category (8) + scenario (8) conventions
/// assert.rs     — performance, AI-specific, and state-leak assertions
/// retry.rs      — exponential backoff utilities
/// security.rs   — security payload scanner
/// stream.rs     — ChatStreamEvent test helpers
/// contract.rs   — AiService contract verifiers
/// observe.rs    — tracing event capture + assertion
/// ```
///
/// # Usage
///
/// Consumer crates add `swebash-test` as a `[dev-dependencies]` entry:
///
/// ```toml
/// [dev-dependencies]
/// swebash-test = { path = "../test" }
/// ```
///
/// Then import the prelude:
///
/// ```ignore
/// use swebash_test::prelude::*;
/// ```

pub mod assert;
pub mod contract;
pub mod error;
pub mod fixture;
pub mod mock;
pub mod naming;
pub mod observe;
pub mod retry;
pub mod security;
pub mod stream;

/// Prelude — import everything commonly needed in tests.
///
/// ```ignore
/// use swebash_test::prelude::*;
/// ```
pub mod prelude {
    pub use crate::assert::{
        assert_ai_error_contains, assert_ai_error_format, assert_ai_error_variant,
        assert_dir_is_empty, assert_env_vars_unset, assert_eventually_consistent,
        assert_latency_p95, assert_latency_p99, assert_setup_error, assert_throughput_above,
        percentile,
    };
    pub use crate::contract::{
        verify_all_contracts, verify_available, verify_current_agent,
        verify_list_agents_non_empty, verify_status, verify_switch_unknown_agent_fails,
        verify_translate_returns_command,
    };
    pub use crate::error::TestError;
    pub use crate::fixture::{ScopedEnvVar, ScopedFixture, ScopedTempDir};
    pub use crate::mock::{
        create_mock_service, create_mock_service_error, create_mock_service_fixed,
        create_mock_service_full_error, mock_config, ErrorMockAiClient, MockAiClient,
        MockEmbedder, MockRecorder,
    };
    pub use crate::naming::{
        suggest_file_name, validate_test_name, validate_test_name_full, TestCategoryKind,
        TestScenarioKind,
    };
    pub use crate::observe::{CapturedEvent, TracingCapture};
    pub use crate::retry::{retry_with_backoff, retry_with_backoff_async};
    pub use crate::security::{
        api_key_leak_payloads, input_validation_payloads, prompt_injection_payloads,
        PayloadCategory, SecurityPayload, SecurityScanResult, SecurityScanner,
    };
    pub use crate::stream::{
        assert_done_event_contains, assert_no_duplication, assert_no_events_after_done,
        collect_stream_events,
    };
}
