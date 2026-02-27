#![forbid(unsafe_code)]

//! Automated interactive test runner for swebash.
//!
//! This crate provides a framework for running structured YAML test specifications
//! against the swebash binary, enabling automated testing of interactive shell behavior.

pub mod driver;
pub mod executor;
pub mod report;
pub mod spec;
pub mod validation;

pub use driver::{Driver, DriverConfig, DriverError, DriverOutput, ToolCallRecord};
pub use executor::{Executor, ExecutorConfig, TestOutcome};
pub use report::{Report, ReportFormat, Reporter};
pub use spec::{TestSpec, TestStep, TestSuite, ValidationRule};
pub use validation::{ValidationError, ValidationResult, Validator};

/// Prelude module for common imports.
pub mod prelude {
    pub use crate::driver::{Driver, DriverConfig, DriverError, DriverOutput, ToolCallRecord};
    pub use crate::executor::{Executor, ExecutorConfig, TestOutcome};
    pub use crate::report::{Report, ReportFormat, Reporter};
    pub use crate::spec::{ExpectConfig, TestSpec, TestStep, TestSuite, ValidationRule};
    pub use crate::validation::{ValidationError, ValidationResult, Validator};
}
