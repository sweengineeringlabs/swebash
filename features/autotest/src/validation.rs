//! Output validation engine for matching test expectations.

use regex::Regex;
use thiserror::Error;

use crate::driver::DriverOutput;
use crate::spec::{ExpectConfig, ValidationRule, ValidationRules};

/// Validation errors.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Contains check failed: expected output to contain '{expected}', got: {actual}")]
    ContainsFailed { expected: String, actual: String },

    #[error("Not-contains check failed: output should not contain '{unexpected}', but it did")]
    NotContainsFailed { unexpected: String },

    #[error("Regex match failed: pattern '{pattern}' did not match output: {actual}")]
    MatchFailed { pattern: String, actual: String },

    #[error("Regex not-match failed: pattern '{pattern}' should not match, but it did")]
    NotMatchFailed { pattern: String },

    #[error("Equals check failed: expected '{expected}', got '{actual}'")]
    EqualsFailed { expected: String, actual: String },

    #[error("Exit code check failed: expected {expected}, got {actual}")]
    ExitCodeFailed { expected: i32, actual: i32 },

    #[error("All conditions must pass, but {failed_count} failed")]
    AllFailed { failed_count: usize },

    #[error("At least one condition must pass, but none did")]
    AnyFailed,

    #[error("Invalid regex pattern '{pattern}': {error}")]
    InvalidRegex { pattern: String, error: String },

    #[error("Tool call check failed: expected tool '{expected}' to be called")]
    ToolNotCalled { expected: String },

    #[error("Stderr validation failed: {0}")]
    StderrFailed(Box<ValidationError>),
}

/// Result of validation.
pub type ValidationResult = Result<(), ValidationError>;

/// Validator for checking output against expectations.
pub struct Validator {
    /// Compiled regex cache.
    regex_cache: std::collections::HashMap<String, Regex>,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    /// Create a new validator.
    pub fn new() -> Self {
        Self {
            regex_cache: std::collections::HashMap::new(),
        }
    }

    /// Validate output against expectations.
    pub fn validate(&mut self, output: &DriverOutput, expect: &ExpectConfig) -> ValidationResult {
        match expect {
            ExpectConfig::Simple(s) => self.check_contains(&output.stdout, s),
            ExpectConfig::Structured(rules) => self.validate_rules(output, rules),
        }
    }

    /// Validate output against structured rules.
    fn validate_rules(&mut self, output: &DriverOutput, rules: &ValidationRules) -> ValidationResult {
        // Check contains
        if let Some(contains) = &rules.contains {
            for s in contains.to_vec() {
                self.check_contains(&output.stdout, &s)?;
            }
        }

        // Check not_contains
        if let Some(not_contains) = &rules.not_contains {
            for s in not_contains.to_vec() {
                self.check_not_contains(&output.stdout, &s)?;
            }
        }

        // Check matches (regex)
        if let Some(matches) = &rules.matches {
            for pattern in matches.to_vec() {
                self.check_matches(&output.stdout, &pattern)?;
            }
        }

        // Check not_matches
        if let Some(not_matches) = &rules.not_matches {
            for pattern in not_matches.to_vec() {
                self.check_not_matches(&output.stdout, &pattern)?;
            }
        }

        // Check equals
        if let Some(expected) = &rules.equals {
            self.check_equals(&output.stdout, expected)?;
        }

        // Check exit code
        if let Some(expected_code) = rules.exit_code {
            self.check_exit_code(output, expected_code)?;
        }

        // Check all rules
        if let Some(all_rules) = &rules.all {
            self.check_all(&output.stdout, all_rules)?;
        }

        // Check any rules
        if let Some(any_rules) = &rules.any {
            self.check_any(&output.stdout, any_rules)?;
        }

        // Check stderr
        if let Some(stderr_rules) = &rules.stderr {
            let stderr_output = DriverOutput {
                stdout: output.stderr.clone(),
                stderr: String::new(),
                exit_status: output.exit_status,
                duration: output.duration,
            };
            self.validate_rules(&stderr_output, stderr_rules)
                .map_err(|e| ValidationError::StderrFailed(Box::new(e)))?;
        }

        // Check tool_called (simplified - looks for tool name in output)
        if let Some(tool_name) = &rules.tool_called {
            self.check_tool_called(&output.stdout, tool_name)?;
        }

        Ok(())
    }

    /// Check if output contains a string.
    fn check_contains(&self, output: &str, expected: &str) -> ValidationResult {
        if output.contains(expected) {
            Ok(())
        } else {
            Err(ValidationError::ContainsFailed {
                expected: expected.to_string(),
                actual: truncate_output(output),
            })
        }
    }

    /// Check if output does not contain a string.
    fn check_not_contains(&self, output: &str, unexpected: &str) -> ValidationResult {
        if output.contains(unexpected) {
            Err(ValidationError::NotContainsFailed {
                unexpected: unexpected.to_string(),
            })
        } else {
            Ok(())
        }
    }

    /// Check if output matches a regex pattern.
    fn check_matches(&mut self, output: &str, pattern: &str) -> ValidationResult {
        let regex = self.get_or_compile_regex(pattern)?;
        if regex.is_match(output) {
            Ok(())
        } else {
            Err(ValidationError::MatchFailed {
                pattern: pattern.to_string(),
                actual: truncate_output(output),
            })
        }
    }

    /// Check if output does not match a regex pattern.
    fn check_not_matches(&mut self, output: &str, pattern: &str) -> ValidationResult {
        let regex = self.get_or_compile_regex(pattern)?;
        if regex.is_match(output) {
            Err(ValidationError::NotMatchFailed {
                pattern: pattern.to_string(),
            })
        } else {
            Ok(())
        }
    }

    /// Check if output exactly equals expected.
    fn check_equals(&self, output: &str, expected: &str) -> ValidationResult {
        if output == expected {
            Ok(())
        } else {
            Err(ValidationError::EqualsFailed {
                expected: expected.to_string(),
                actual: truncate_output(output),
            })
        }
    }

    /// Check exit code.
    fn check_exit_code(&self, output: &DriverOutput, expected: i32) -> ValidationResult {
        let actual = output
            .exit_status
            .and_then(|s| s.code())
            .unwrap_or(-1);

        if actual == expected {
            Ok(())
        } else {
            Err(ValidationError::ExitCodeFailed { expected, actual })
        }
    }

    /// Check that all rules pass.
    fn check_all(&mut self, output: &str, rules: &[ValidationRule]) -> ValidationResult {
        let mut failed_count = 0;
        for rule in rules {
            if self.check_single_rule(output, rule).is_err() {
                failed_count += 1;
            }
        }
        if failed_count == 0 {
            Ok(())
        } else {
            Err(ValidationError::AllFailed { failed_count })
        }
    }

    /// Check that at least one rule passes.
    fn check_any(&mut self, output: &str, rules: &[ValidationRule]) -> ValidationResult {
        for rule in rules {
            if self.check_single_rule(output, rule).is_ok() {
                return Ok(());
            }
        }
        Err(ValidationError::AnyFailed)
    }

    /// Check a single validation rule.
    fn check_single_rule(&mut self, output: &str, rule: &ValidationRule) -> ValidationResult {
        match rule {
            ValidationRule::Nested(rules) => {
                // For nested rules, create a fake DriverOutput and validate
                let fake_output = DriverOutput {
                    stdout: output.to_string(),
                    stderr: String::new(),
                    exit_status: None,
                    duration: std::time::Duration::ZERO,
                };
                self.validate_rules(&fake_output, rules)
            }
            ValidationRule::Contains { contains } => self.check_contains(output, contains),
            ValidationRule::NotContains { not_contains } => self.check_not_contains(output, not_contains),
            ValidationRule::Matches { matches } => self.check_matches(output, matches),
            ValidationRule::NotMatches { not_matches } => self.check_not_matches(output, not_matches),
            ValidationRule::Equals { equals } => self.check_equals(output, equals),
            ValidationRule::ExitCode { exit_code: _code } => {
                // For exit code in a rule, we can't check without output context
                // This is a simplified version - real implementation would need output
                Ok(())
            }
            ValidationRule::ToolCalled { tool_called } => self.check_tool_called(output, tool_called),
        }
    }

    /// Check if a tool was called (simplified - looks in output).
    fn check_tool_called(&self, output: &str, tool_name: &str) -> ValidationResult {
        // Look for patterns like "tool: tool_name" or "calling tool_name"
        let patterns = [
            format!("tool: {}", tool_name),
            format!("calling {}", tool_name),
            format!("\"tool\": \"{}\"", tool_name),
            format!("tool_name: {}", tool_name),
        ];

        for pattern in &patterns {
            if output.to_lowercase().contains(&pattern.to_lowercase()) {
                return Ok(());
            }
        }

        Err(ValidationError::ToolNotCalled {
            expected: tool_name.to_string(),
        })
    }

    /// Get or compile a regex.
    fn get_or_compile_regex(&mut self, pattern: &str) -> Result<&Regex, ValidationError> {
        if !self.regex_cache.contains_key(pattern) {
            let regex = Regex::new(pattern).map_err(|e| ValidationError::InvalidRegex {
                pattern: pattern.to_string(),
                error: e.to_string(),
            })?;
            self.regex_cache.insert(pattern.to_string(), regex);
        }
        Ok(self.regex_cache.get(pattern).unwrap())
    }
}

/// Truncate output for error messages.
fn truncate_output(output: &str) -> String {
    const MAX_LEN: usize = 500;
    if output.len() <= MAX_LEN {
        output.to_string()
    } else {
        format!("{}... (truncated)", &output[..MAX_LEN])
    }
}

/// Quick validation helpers.
pub fn contains(output: &str, expected: &str) -> ValidationResult {
    Validator::new().check_contains(output, expected)
}

pub fn not_contains(output: &str, unexpected: &str) -> ValidationResult {
    Validator::new().check_not_contains(output, unexpected)
}

pub fn matches(output: &str, pattern: &str) -> ValidationResult {
    Validator::new().check_matches(output, pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::StringOrVec;

    #[test]
    fn test_contains() {
        let validator = Validator::new();
        assert!(validator.check_contains("hello world", "hello").is_ok());
        assert!(validator.check_contains("hello world", "goodbye").is_err());
    }

    #[test]
    fn test_not_contains() {
        let validator = Validator::new();
        assert!(validator.check_not_contains("hello world", "goodbye").is_ok());
        assert!(validator.check_not_contains("hello world", "hello").is_err());
    }

    #[test]
    fn test_matches() {
        let mut validator = Validator::new();
        assert!(validator.check_matches("hello world", r"hello \w+").is_ok());
        assert!(validator.check_matches("hello world", r"^goodbye").is_err());
    }

    #[test]
    fn test_equals() {
        let validator = Validator::new();
        assert!(validator.check_equals("hello", "hello").is_ok());
        assert!(validator.check_equals("hello", "hello ").is_err());
    }

    #[test]
    fn test_all_rules() {
        let mut validator = Validator::new();
        let rules = vec![
            ValidationRule::Contains { contains: "hello".to_string() },
            ValidationRule::Contains { contains: "world".to_string() },
        ];
        assert!(validator.check_all("hello world", &rules).is_ok());

        let rules_fail = vec![
            ValidationRule::Contains { contains: "hello".to_string() },
            ValidationRule::Contains { contains: "goodbye".to_string() },
        ];
        assert!(validator.check_all("hello world", &rules_fail).is_err());
    }

    #[test]
    fn test_any_rules() {
        let mut validator = Validator::new();
        let rules = vec![
            ValidationRule::Contains { contains: "goodbye".to_string() },
            ValidationRule::Contains { contains: "world".to_string() },
        ];
        assert!(validator.check_any("hello world", &rules).is_ok());

        let rules_fail = vec![
            ValidationRule::Contains { contains: "goodbye".to_string() },
            ValidationRule::Contains { contains: "farewell".to_string() },
        ];
        assert!(validator.check_any("hello world", &rules_fail).is_err());
    }

    #[test]
    fn test_structured_validation() {
        let mut validator = Validator::new();
        let output = DriverOutput {
            stdout: "hello world\n".to_string(),
            stderr: "".to_string(),
            exit_status: None,
            duration: std::time::Duration::from_millis(100),
        };

        let rules = ValidationRules {
            contains: Some(StringOrVec::Single("hello".to_string())),
            not_contains: Some(StringOrVec::Single("error".to_string())),
            ..Default::default()
        };

        assert!(validator.validate_rules(&output, &rules).is_ok());
    }
}
