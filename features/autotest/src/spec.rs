//! Test specification schema and YAML parsing.
//!
//! Defines the structure of YAML test files and provides parsing functionality.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Root structure of a test suite YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    /// Schema version (currently 1).
    #[serde(default = "default_version")]
    pub version: u32,

    /// Suite identifier (e.g., "shell_basics").
    pub suite: String,

    /// Suite-level configuration.
    #[serde(default)]
    pub config: SuiteConfig,

    /// List of test specifications.
    pub tests: Vec<TestSpec>,
}

fn default_version() -> u32 {
    1
}

/// Suite-level configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SuiteConfig {
    /// Default timeout for all tests in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Environment variables to set for all tests.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Setup commands to run before each test.
    #[serde(default)]
    pub setup: Vec<String>,

    /// Teardown commands to run after each test.
    #[serde(default)]
    pub teardown: Vec<String>,

    /// Tags that apply to all tests in the suite.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether tests in this suite can run in parallel.
    #[serde(default = "default_parallel")]
    pub parallel: bool,
}

fn default_timeout_ms() -> u64 {
    30000
}

fn default_parallel() -> bool {
    true
}

/// A single test specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSpec {
    /// Unique test identifier within the suite.
    pub id: String,

    /// Human-readable test name/description.
    pub name: String,

    /// Test-specific configuration (overrides suite config).
    #[serde(default)]
    pub config: TestConfig,

    /// Tags for filtering tests.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether this test is currently skipped.
    #[serde(default)]
    pub skip: bool,

    /// Reason for skipping (if skip is true).
    #[serde(default)]
    pub skip_reason: Option<String>,

    /// Test steps to execute.
    pub steps: Vec<TestStep>,
}

/// Test-specific configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestConfig {
    /// Timeout for this specific test in milliseconds.
    pub timeout_ms: Option<u64>,

    /// Additional environment variables for this test.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Setup commands for this specific test.
    #[serde(default)]
    pub setup: Vec<String>,

    /// Teardown commands for this specific test.
    #[serde(default)]
    pub teardown: Vec<String>,

    /// Whether this test requires a clean temp directory.
    #[serde(default)]
    pub clean_temp: bool,

    /// Working directory (relative to temp dir if clean_temp is true).
    pub cwd: Option<String>,
}

/// A single step within a test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    /// Command to send to the shell.
    pub command: String,

    /// Expected output validation rules.
    #[serde(default)]
    pub expect: Option<ExpectConfig>,

    /// Timeout for this specific step in milliseconds.
    pub timeout_ms: Option<u64>,

    /// Delay before sending command (milliseconds).
    #[serde(default)]
    pub delay_ms: Option<u64>,

    /// Whether to wait for specific output before proceeding.
    pub wait_for: Option<String>,

    /// Description of what this step does.
    pub description: Option<String>,
}

/// Validation rules for expected output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectConfig {
    /// Simple string match (contains).
    Simple(String),

    /// Structured validation rules.
    Structured(ValidationRules),
}

/// Structured validation rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationRules {
    /// Output must contain this string.
    pub contains: Option<StringOrVec>,

    /// Output must not contain this string.
    pub not_contains: Option<StringOrVec>,

    /// Output must match this regex pattern.
    pub matches: Option<StringOrVec>,

    /// Output must not match this regex pattern.
    pub not_matches: Option<StringOrVec>,

    /// Output must exactly equal this string.
    pub equals: Option<String>,

    /// All of these rules must pass.
    pub all: Option<Vec<ValidationRule>>,

    /// At least one of these rules must pass.
    pub any: Option<Vec<ValidationRule>>,

    /// Exit code must equal this value.
    pub exit_code: Option<i32>,

    /// Stderr must satisfy these rules.
    pub stderr: Option<Box<ValidationRules>>,

    /// A specific tool must have been called (for AI tests).
    pub tool_called: Option<String>,

    /// Tool call must have these parameters.
    pub tool_params: Option<HashMap<String, serde_yaml::Value>>,
}

/// Either a single string or a vector of strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrVec {
    /// Convert to a vector of strings.
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            StringOrVec::Single(s) => vec![s.clone()],
            StringOrVec::Multiple(v) => v.clone(),
        }
    }
}

/// A single validation rule (used in all/any combinators).
/// Supports nested all/any for complex logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValidationRule {
    /// Nested validation rules (supports all fields).
    Nested(Box<ValidationRules>),

    /// Simple contains check.
    Contains {
        contains: String,
    },

    /// Simple not_contains check.
    NotContains {
        not_contains: String,
    },

    /// Simple matches check.
    Matches {
        matches: String,
    },

    /// Simple not_matches check.
    NotMatches {
        not_matches: String,
    },

    /// Simple equals check.
    Equals {
        equals: String,
    },

    /// Exit code check.
    ExitCode {
        exit_code: i32,
    },

    /// Tool called check.
    ToolCalled {
        tool_called: String,
    },
}

impl TestSuite {
    /// Parse a test suite from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| format!("Failed to parse test suite YAML: {e}"))
    }

    /// Load a test suite from a YAML file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read test suite file {}: {e}", path.display()))?;
        Self::from_yaml(&content)
    }

    /// Get all tests, optionally filtered by tags.
    pub fn tests_with_tags(&self, include_tags: &[String], exclude_tags: &[String]) -> Vec<&TestSpec> {
        self.tests
            .iter()
            .filter(|test| {
                // Skip tests marked as skipped
                if test.skip {
                    return false;
                }

                // Combine suite tags with test tags
                let all_tags: Vec<&str> = self
                    .config
                    .tags
                    .iter()
                    .chain(test.tags.iter())
                    .map(|s| s.as_str())
                    .collect();

                // If include_tags is specified, test must have at least one
                if !include_tags.is_empty() {
                    let has_include = include_tags
                        .iter()
                        .any(|t| all_tags.contains(&t.as_str()));
                    if !has_include {
                        return false;
                    }
                }

                // Test must not have any excluded tags
                let has_exclude = exclude_tags
                    .iter()
                    .any(|t| all_tags.contains(&t.as_str()));
                !has_exclude
            })
            .collect()
    }

    /// Get effective timeout for a test (test override > suite default).
    pub fn effective_timeout(&self, test: &TestSpec) -> u64 {
        test.config.timeout_ms.unwrap_or(self.config.timeout_ms)
    }

    /// Get combined environment variables for a test.
    pub fn effective_env(&self, test: &TestSpec) -> HashMap<String, String> {
        let mut env = self.config.env.clone();
        env.extend(test.config.env.clone());
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_suite() {
        let yaml = r#"
suite: test_suite
tests:
  - id: test_1
    name: "Test One"
    steps:
      - command: "echo hello"
"#;
        let suite = TestSuite::from_yaml(yaml).unwrap();
        assert_eq!(suite.suite, "test_suite");
        assert_eq!(suite.tests.len(), 1);
        assert_eq!(suite.tests[0].id, "test_1");
    }

    #[test]
    fn parse_full_suite() {
        let yaml = r#"
version: 1
suite: shell_basics

config:
  timeout_ms: 30000
  env:
    SWEBASH_AI_ENABLED: "true"
  setup:
    - mkdir -p /tmp/test_workspace
  teardown:
    - rm -rf /tmp/test_workspace

tests:
  - id: echo_simple
    name: "Echo prints arguments"
    steps:
      - command: "echo hello"
        expect:
          contains: "hello"

  - id: ai_ask_command
    name: "AI suggests command"
    config:
      timeout_ms: 60000
    steps:
      - command: "ai ask list files"
        expect:
          any:
            - contains: "ls"
            - contains: "dir"
      - command: "n"
"#;
        let suite = TestSuite::from_yaml(yaml).unwrap();
        assert_eq!(suite.version, 1);
        assert_eq!(suite.suite, "shell_basics");
        assert_eq!(suite.config.timeout_ms, 30000);
        assert_eq!(suite.tests.len(), 2);

        // Check first test
        let test1 = &suite.tests[0];
        assert_eq!(test1.id, "echo_simple");
        assert_eq!(test1.steps.len(), 1);

        // Check second test with override
        let test2 = &suite.tests[1];
        assert_eq!(test2.id, "ai_ask_command");
        assert_eq!(test2.config.timeout_ms, Some(60000));
        assert_eq!(test2.steps.len(), 2);
    }

    #[test]
    fn parse_validation_rules() {
        let yaml = r#"
suite: validation_test
tests:
  - id: complex_validation
    name: "Complex validation"
    steps:
      - command: "echo test"
        expect:
          contains: "test"
          not_contains: "error"
          matches: "^test$"
"#;
        let suite = TestSuite::from_yaml(yaml).unwrap();
        let step = &suite.tests[0].steps[0];

        if let Some(ExpectConfig::Structured(rules)) = &step.expect {
            assert!(rules.contains.is_some());
            assert!(rules.not_contains.is_some());
            assert!(rules.matches.is_some());
        } else {
            panic!("Expected structured validation rules");
        }
    }

    #[test]
    fn filter_tests_by_tags() {
        let yaml = r#"
suite: tag_test
config:
  tags: [smoke]
tests:
  - id: test_1
    name: "Test 1"
    tags: [fast]
    steps:
      - command: "echo 1"
  - id: test_2
    name: "Test 2"
    tags: [slow, flaky]
    steps:
      - command: "echo 2"
  - id: test_3
    name: "Test 3"
    skip: true
    steps:
      - command: "echo 3"
"#;
        let suite = TestSuite::from_yaml(yaml).unwrap();

        // Include fast
        let fast_tests = suite.tests_with_tags(&["fast".to_string()], &[]);
        assert_eq!(fast_tests.len(), 1);
        assert_eq!(fast_tests[0].id, "test_1");

        // Exclude flaky
        let non_flaky = suite.tests_with_tags(&[], &["flaky".to_string()]);
        assert_eq!(non_flaky.len(), 1);
        assert_eq!(non_flaky[0].id, "test_1");

        // Include smoke (from suite config)
        let smoke_tests = suite.tests_with_tags(&["smoke".to_string()], &[]);
        assert_eq!(smoke_tests.len(), 2);

        // All tests (skipped ones are excluded)
        let all_tests = suite.tests_with_tags(&[], &[]);
        assert_eq!(all_tests.len(), 2);
    }

    #[test]
    fn effective_config() {
        let yaml = r#"
suite: config_test
config:
  timeout_ms: 5000
  env:
    A: "1"
    B: "2"
tests:
  - id: test_1
    name: "Test 1"
    config:
      timeout_ms: 10000
      env:
        B: "override"
        C: "3"
    steps:
      - command: "echo"
"#;
        let suite = TestSuite::from_yaml(yaml).unwrap();
        let test = &suite.tests[0];

        assert_eq!(suite.effective_timeout(test), 10000);

        let env = suite.effective_env(test);
        assert_eq!(env.get("A"), Some(&"1".to_string()));
        assert_eq!(env.get("B"), Some(&"override".to_string()));
        assert_eq!(env.get("C"), Some(&"3".to_string()));
    }
}
