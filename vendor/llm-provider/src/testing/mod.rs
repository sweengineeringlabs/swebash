//! Testing utilities for llm-provider
//!
//! Provides [`MockLlmService`] for tests that need an `LlmService` without
//! making real API calls.
//!
//! Gated behind `#[cfg(any(test, feature = "testing"))]`.

pub mod mock_service;

pub use mock_service::{MockBehaviour, MockLlmService};
