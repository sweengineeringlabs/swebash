/// Test naming conventions and category classification.
///
/// Defines 8 test categories following the rustboot testing strategy,
/// with file naming, function prefix, feature gate, and CI cadence.

use crate::error::TestError;

/// The 8 test categories in the testing pyramid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestCategoryKind {
    /// Unit tests: inline `#[cfg(test)]` in source files.
    Unit,
    /// Feature tests: inline `#[cfg(test)]` testing specific features.
    Feature,
    /// Integration tests: external test files exercising public APIs.
    Integration,
    /// Stress tests: concurrent load, race conditions, resource contention.
    Stress,
    /// Performance tests: latency benchmarks, throughput measurement.
    Performance,
    /// Load tests: sustained volume, memory growth, degradation.
    Load,
    /// End-to-end tests: full system with real or live dependencies.
    E2e,
    /// Security tests: injection, leak detection, input validation.
    Security,
}

impl TestCategoryKind {
    /// The file suffix for external test files (e.g., `_int_test`).
    ///
    /// Unit and Feature tests are inline and have no file suffix.
    pub fn file_suffix(&self) -> Option<&'static str> {
        match self {
            Self::Unit | Self::Feature => None,
            Self::Integration => Some("_int_test"),
            Self::Stress => Some("_stress_test"),
            Self::Performance => Some("_perf_test"),
            Self::Load => Some("_load_test"),
            Self::E2e => Some("_e2e_test"),
            Self::Security => Some("_security_test"),
        }
    }

    /// The required function name prefix for this category.
    ///
    /// Unit, Feature, and Integration tests have no required prefix.
    pub fn function_prefix(&self) -> Option<&'static str> {
        match self {
            Self::Unit | Self::Feature | Self::Integration => None,
            Self::Stress => Some("stress_test_"),
            Self::Performance => Some("perf_"),
            Self::Load => Some("load_"),
            Self::E2e => Some("e2e_"),
            Self::Security => Some("security_"),
        }
    }

    /// The Cargo feature gate required to compile these tests.
    ///
    /// `None` means the tests compile unconditionally.
    pub fn feature_gate(&self) -> Option<&'static str> {
        match self {
            Self::Unit | Self::Feature | Self::Integration | Self::Security => None,
            Self::Stress => Some("stress"),
            Self::Performance => Some("perf"),
            Self::Load => Some("load"),
            Self::E2e => Some("live"),
        }
    }

    /// CI cadence label for this test category.
    pub fn ci_cadence(&self) -> &'static str {
        match self {
            Self::Unit | Self::Feature | Self::Integration | Self::Security => "Every commit",
            Self::Stress | Self::Performance => "Nightly",
            Self::Load => "Weekly",
            Self::E2e => "Integration gate",
        }
    }

    /// All category variants.
    pub fn all() -> &'static [TestCategoryKind] {
        &[
            Self::Unit,
            Self::Feature,
            Self::Integration,
            Self::Stress,
            Self::Performance,
            Self::Load,
            Self::E2e,
            Self::Security,
        ]
    }
}

/// Validate a test function name against its category's naming convention.
///
/// Returns `Ok(())` if the name is valid, or `Err(TestError::NamingViolation)`
/// with a descriptive message if it violates the convention.
pub fn validate_test_name(name: &str, category: TestCategoryKind) -> Result<(), TestError> {
    if name.is_empty() {
        return Err(TestError::NamingViolation("test name must not be empty".into()));
    }

    if let Some(prefix) = category.function_prefix() {
        if !name.starts_with(prefix) {
            return Err(TestError::NamingViolation(format!(
                "{category:?} test '{name}' must start with '{prefix}'"
            )));
        }
    }

    Ok(())
}

/// Suggest the test file name for a given crate and category.
///
/// Returns `None` for Unit and Feature categories (they are inline).
///
/// # Example
///
/// ```
/// use swebash_test::naming::{suggest_file_name, TestCategoryKind};
///
/// assert_eq!(
///     suggest_file_name("ai", TestCategoryKind::Integration),
///     Some("ai_int_test.rs".to_string())
/// );
/// assert_eq!(
///     suggest_file_name("ai", TestCategoryKind::Unit),
///     None
/// );
/// ```
pub fn suggest_file_name(crate_name: &str, category: TestCategoryKind) -> Option<String> {
    category
        .file_suffix()
        .map(|suffix| format!("{crate_name}{suffix}.rs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_categories_returns_eight() {
        assert_eq!(TestCategoryKind::all().len(), 8);
    }

    #[test]
    fn unit_has_no_file_suffix() {
        assert_eq!(TestCategoryKind::Unit.file_suffix(), None);
    }

    #[test]
    fn integration_has_int_test_suffix() {
        assert_eq!(
            TestCategoryKind::Integration.file_suffix(),
            Some("_int_test")
        );
    }

    #[test]
    fn stress_has_stress_test_suffix() {
        assert_eq!(
            TestCategoryKind::Stress.file_suffix(),
            Some("_stress_test")
        );
    }

    #[test]
    fn performance_has_perf_test_suffix() {
        assert_eq!(
            TestCategoryKind::Performance.file_suffix(),
            Some("_perf_test")
        );
    }

    #[test]
    fn load_has_load_test_suffix() {
        assert_eq!(TestCategoryKind::Load.file_suffix(), Some("_load_test"));
    }

    #[test]
    fn e2e_has_e2e_test_suffix() {
        assert_eq!(TestCategoryKind::E2e.file_suffix(), Some("_e2e_test"));
    }

    #[test]
    fn security_has_security_test_suffix() {
        assert_eq!(
            TestCategoryKind::Security.file_suffix(),
            Some("_security_test")
        );
    }

    #[test]
    fn unit_has_no_function_prefix() {
        assert_eq!(TestCategoryKind::Unit.function_prefix(), None);
    }

    #[test]
    fn stress_requires_stress_test_prefix() {
        assert_eq!(
            TestCategoryKind::Stress.function_prefix(),
            Some("stress_test_")
        );
    }

    #[test]
    fn performance_requires_perf_prefix() {
        assert_eq!(
            TestCategoryKind::Performance.function_prefix(),
            Some("perf_")
        );
    }

    #[test]
    fn e2e_requires_e2e_prefix() {
        assert_eq!(TestCategoryKind::E2e.function_prefix(), Some("e2e_"));
    }

    #[test]
    fn security_requires_security_prefix() {
        assert_eq!(
            TestCategoryKind::Security.function_prefix(),
            Some("security_")
        );
    }

    #[test]
    fn unit_has_no_feature_gate() {
        assert_eq!(TestCategoryKind::Unit.feature_gate(), None);
    }

    #[test]
    fn stress_requires_stress_gate() {
        assert_eq!(TestCategoryKind::Stress.feature_gate(), Some("stress"));
    }

    #[test]
    fn performance_requires_perf_gate() {
        assert_eq!(TestCategoryKind::Performance.feature_gate(), Some("perf"));
    }

    #[test]
    fn load_requires_load_gate() {
        assert_eq!(TestCategoryKind::Load.feature_gate(), Some("load"));
    }

    #[test]
    fn e2e_requires_live_gate() {
        assert_eq!(TestCategoryKind::E2e.feature_gate(), Some("live"));
    }

    #[test]
    fn security_has_no_feature_gate() {
        assert_eq!(TestCategoryKind::Security.feature_gate(), None);
    }

    #[test]
    fn ci_cadences_are_correct() {
        assert_eq!(TestCategoryKind::Unit.ci_cadence(), "Every commit");
        assert_eq!(TestCategoryKind::Stress.ci_cadence(), "Nightly");
        assert_eq!(TestCategoryKind::Load.ci_cadence(), "Weekly");
        assert_eq!(TestCategoryKind::E2e.ci_cadence(), "Integration gate");
    }

    #[test]
    fn validate_name_accepts_valid_stress_test() {
        assert!(validate_test_name("stress_test_concurrent_sessions", TestCategoryKind::Stress).is_ok());
    }

    #[test]
    fn validate_name_rejects_invalid_stress_test() {
        let result = validate_test_name("my_test", TestCategoryKind::Stress);
        match result {
            Err(TestError::NamingViolation(msg)) => {
                assert!(msg.contains("stress_test_"));
            }
            other => panic!("Expected NamingViolation, got: {other:?}"),
        }
    }

    #[test]
    fn validate_name_accepts_integration_without_prefix() {
        assert!(validate_test_name("chat_returns_reply", TestCategoryKind::Integration).is_ok());
    }

    #[test]
    fn validate_name_rejects_empty() {
        let result = validate_test_name("", TestCategoryKind::Unit);
        assert!(matches!(result, Err(TestError::NamingViolation(_))));
    }

    #[test]
    fn suggest_file_name_integration() {
        assert_eq!(
            suggest_file_name("ai", TestCategoryKind::Integration),
            Some("ai_int_test.rs".into())
        );
    }

    #[test]
    fn suggest_file_name_stress() {
        assert_eq!(
            suggest_file_name("ai", TestCategoryKind::Stress),
            Some("ai_stress_test.rs".into())
        );
    }

    #[test]
    fn suggest_file_name_unit_returns_none() {
        assert_eq!(suggest_file_name("ai", TestCategoryKind::Unit), None);
    }

    #[test]
    fn suggest_file_name_security() {
        assert_eq!(
            suggest_file_name("ai", TestCategoryKind::Security),
            Some("ai_security_test.rs".into())
        );
    }
}
