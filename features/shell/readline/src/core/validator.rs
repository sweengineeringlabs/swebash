/// Validator for multi-line editing
pub struct Validator;

impl Validator {
    pub fn new() -> Self {
        Self
    }

    /// Check if input is complete or needs more lines
    pub fn validate(&self, input: &str) -> ValidationResult {
        // Check for trailing backslash
        if input.trim_end().ends_with('\\') {
            return ValidationResult::Incomplete;
        }

        // Check for unclosed quotes
        let mut in_quote = false;
        let mut quote_char = '\0';
        let mut escape = false;

        for ch in input.chars() {
            if escape {
                escape = false;
                continue;
            }

            match ch {
                '\\' => {
                    escape = true;
                }
                '"' | '\'' if !in_quote => {
                    in_quote = true;
                    quote_char = ch;
                }
                c if in_quote && c == quote_char => {
                    in_quote = false;
                }
                _ => {}
            }
        }

        if in_quote {
            return ValidationResult::Incomplete;
        }

        // Check for unclosed brackets (simple heuristic)
        let open_parens = input.chars().filter(|&c| c == '(').count();
        let close_parens = input.chars().filter(|&c| c == ')').count();
        if open_parens != close_parens {
            return ValidationResult::Incomplete;
        }

        let open_braces = input.chars().filter(|&c| c == '{').count();
        let close_braces = input.chars().filter(|&c| c == '}').count();
        if open_braces != close_braces {
            return ValidationResult::Incomplete;
        }

        ValidationResult::Complete
    }
}

#[derive(Debug, PartialEq)]
pub enum ValidationResult {
    Complete,
    Incomplete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_command() {
        let validator = Validator::new();
        assert_eq!(validator.validate("echo hello"), ValidationResult::Complete);
    }

    #[test]
    fn test_incomplete_backslash() {
        let validator = Validator::new();
        assert_eq!(
            validator.validate("echo hello \\"),
            ValidationResult::Incomplete
        );
    }

    #[test]
    fn test_incomplete_quote() {
        let validator = Validator::new();
        assert_eq!(
            validator.validate("echo \"hello"),
            ValidationResult::Incomplete
        );
    }

    #[test]
    fn test_complete_quoted() {
        let validator = Validator::new();
        assert_eq!(
            validator.validate("echo \"hello world\""),
            ValidationResult::Complete
        );
    }

    #[test]
    fn test_incomplete_parens() {
        let validator = Validator::new();
        assert_eq!(validator.validate("echo (test"), ValidationResult::Incomplete);
    }

    #[test]
    fn test_complete_parens() {
        let validator = Validator::new();
        assert_eq!(
            validator.validate("echo (test)"),
            ValidationResult::Complete
        );
    }
}
