//! Test execution orchestrator.
//!
//! Handles running test suites and individual tests, managing setup/teardown,
//! and collecting results.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::driver::{Driver, DriverBuilder, DriverError, DriverOutput};
use crate::spec::{TestSpec, TestStep, TestSuite};
use crate::validation::Validator;

/// Test execution result.
#[derive(Debug, Clone)]
pub enum TestOutcome {
    /// Test passed.
    Passed {
        duration: Duration,
    },

    /// Test failed.
    Failed {
        duration: Duration,
        error: String,
        output: Option<DriverOutput>,
        step_index: Option<usize>,
    },

    /// Test was skipped.
    Skipped {
        reason: Option<String>,
    },

    /// Test encountered an error during setup/execution.
    Error {
        error: String,
    },
}

impl TestOutcome {
    /// Check if the outcome is a pass.
    pub fn is_passed(&self) -> bool {
        matches!(self, TestOutcome::Passed { .. })
    }

    /// Check if the outcome is a failure.
    pub fn is_failed(&self) -> bool {
        matches!(self, TestOutcome::Failed { .. })
    }

    /// Check if the outcome is skipped.
    pub fn is_skipped(&self) -> bool {
        matches!(self, TestOutcome::Skipped { .. })
    }

    /// Get duration if available.
    pub fn duration(&self) -> Option<Duration> {
        match self {
            TestOutcome::Passed { duration } => Some(*duration),
            TestOutcome::Failed { duration, .. } => Some(*duration),
            _ => None,
        }
    }
}

/// Result of executing a single test.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Test ID.
    pub test_id: String,

    /// Test name.
    pub test_name: String,

    /// Suite name.
    pub suite_name: String,

    /// Test outcome.
    pub outcome: TestOutcome,
}

/// Result of executing a test suite.
#[derive(Debug, Clone)]
pub struct SuiteResult {
    /// Suite name.
    pub suite_name: String,

    /// Individual test results.
    pub test_results: Vec<TestResult>,

    /// Total duration.
    pub duration: Duration,
}

impl SuiteResult {
    /// Count of passed tests.
    pub fn passed_count(&self) -> usize {
        self.test_results.iter().filter(|r| r.outcome.is_passed()).count()
    }

    /// Count of failed tests.
    pub fn failed_count(&self) -> usize {
        self.test_results.iter().filter(|r| r.outcome.is_failed()).count()
    }

    /// Count of skipped tests.
    pub fn skipped_count(&self) -> usize {
        self.test_results.iter().filter(|r| r.outcome.is_skipped()).count()
    }

    /// Total test count.
    pub fn total_count(&self) -> usize {
        self.test_results.len()
    }

    /// Check if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.failed_count() == 0
    }
}

/// Configuration for the executor.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Path to the swebash binary.
    pub binary_path: Option<PathBuf>,

    /// Base temp directory for test workspaces.
    pub temp_dir: Option<PathBuf>,

    /// Tags to include (empty = all).
    pub include_tags: Vec<String>,

    /// Tags to exclude.
    pub exclude_tags: Vec<String>,

    /// Number of parallel workers.
    pub parallel_workers: usize,

    /// Whether to run tests in parallel.
    pub parallel: bool,

    /// Verbose output.
    pub verbose: bool,

    /// Continue on failure.
    pub continue_on_failure: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            binary_path: None,
            temp_dir: None,
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            parallel_workers: num_cpus::get().min(8),
            parallel: true,
            verbose: false,
            continue_on_failure: true,
        }
    }
}

/// Test executor.
pub struct Executor {
    config: ExecutorConfig,
}

impl Executor {
    /// Create a new executor with the given configuration.
    pub fn new(config: ExecutorConfig) -> Self {
        Self { config }
    }

    /// Create an executor with default configuration.
    pub fn default_executor() -> Self {
        Self::new(ExecutorConfig::default())
    }

    /// Execute a test suite.
    pub fn execute_suite(&self, suite: &TestSuite) -> SuiteResult {
        let start = Instant::now();

        // Filter tests by tags
        let tests = suite.tests_with_tags(&self.config.include_tags, &self.config.exclude_tags);

        // Execute tests (parallel or sequential)
        let test_results = if self.config.parallel && suite.config.parallel {
            self.execute_tests_parallel(suite, &tests)
        } else {
            self.execute_tests_sequential(suite, &tests)
        };

        SuiteResult {
            suite_name: suite.suite.clone(),
            test_results,
            duration: start.elapsed(),
        }
    }

    /// Execute a single test.
    pub fn execute_test(&self, suite: &TestSuite, test: &TestSpec) -> TestResult {
        // Check if skipped
        if test.skip {
            return TestResult {
                test_id: test.id.clone(),
                test_name: test.name.clone(),
                suite_name: suite.suite.clone(),
                outcome: TestOutcome::Skipped {
                    reason: test.skip_reason.clone(),
                },
            };
        }

        let start = Instant::now();

        // Create temp directory if needed
        let temp_dir = if test.config.clean_temp {
            Some(self.create_temp_dir(&test.id))
        } else {
            None
        };

        let working_dir = temp_dir
            .as_ref()
            .map(|d| d.path().to_path_buf())
            .or_else(|| test.config.cwd.as_ref().map(PathBuf::from))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        // Build driver
        let driver_result = self.build_driver(suite, test, &working_dir);
        let driver = match driver_result {
            Ok(d) => d,
            Err(e) => {
                return TestResult {
                    test_id: test.id.clone(),
                    test_name: test.name.clone(),
                    suite_name: suite.suite.clone(),
                    outcome: TestOutcome::Error {
                        error: format!("Failed to create driver: {}", e),
                    },
                };
            }
        };

        // Execute all steps in a single session (setup + test + teardown)
        let mut validator = Validator::new();
        let step_result = self.execute_all_steps(suite, test, &driver, &mut validator);

        // Temp dir cleanup happens automatically on drop

        let duration = start.elapsed();

        match step_result {
            Ok(_) => TestResult {
                test_id: test.id.clone(),
                test_name: test.name.clone(),
                suite_name: suite.suite.clone(),
                outcome: TestOutcome::Passed { duration },
            },
            Err((error, output, step_index)) => TestResult {
                test_id: test.id.clone(),
                test_name: test.name.clone(),
                suite_name: suite.suite.clone(),
                outcome: TestOutcome::Failed {
                    duration,
                    error,
                    output: Some(output),
                    step_index: Some(step_index),
                },
            },
        }
    }

    /// Execute tests in parallel.
    fn execute_tests_parallel(
        &self,
        suite: &TestSuite,
        tests: &[&TestSpec],
    ) -> Vec<TestResult> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.config.parallel_workers)
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

        pool.install(|| {
            tests
                .par_iter()
                .map(|test| self.execute_test(suite, test))
                .collect()
        })
    }

    /// Execute tests sequentially.
    fn execute_tests_sequential(
        &self,
        suite: &TestSuite,
        tests: &[&TestSpec],
    ) -> Vec<TestResult> {
        let mut results = Vec::with_capacity(tests.len());
        for test in tests {
            let result = self.execute_test(suite, test);
            let should_continue = self.config.continue_on_failure || result.outcome.is_passed();
            results.push(result);
            if !should_continue {
                break;
            }
        }
        results
    }

    /// Build a driver for a test.
    fn build_driver(
        &self,
        suite: &TestSuite,
        test: &TestSpec,
        working_dir: &Path,
    ) -> Result<Driver, DriverError> {
        let mut builder = DriverBuilder::new()
            .working_dir(working_dir.to_path_buf())
            .timeout(Duration::from_millis(suite.effective_timeout(test)));

        // Set binary path if specified
        if let Some(binary_path) = &self.config.binary_path {
            builder = builder.binary_path(binary_path.clone());
        }

        // Set environment variables
        let env = suite.effective_env(test);
        builder = builder.envs(env);

        // Set workspace to working dir
        builder = builder.workspace(working_dir.to_path_buf());

        builder.build()
    }

    /// Execute all steps (setup + test + teardown) in a single shell session.
    fn execute_all_steps(
        &self,
        suite: &TestSuite,
        test: &TestSpec,
        driver: &Driver,
        validator: &mut Validator,
    ) -> Result<(), (String, DriverOutput, usize)> {
        // Collect all commands: suite setup + test setup + test steps + test teardown + suite teardown
        let mut commands: Vec<&str> = Vec::new();

        // Suite setup
        for cmd in &suite.config.setup {
            commands.push(cmd.as_str());
        }

        // Test setup
        for cmd in &test.config.setup {
            commands.push(cmd.as_str());
        }

        // Test steps
        let step_start_idx = commands.len();
        for step in &test.steps {
            commands.push(step.command.as_str());
        }

        // Test teardown
        for cmd in &test.config.teardown {
            commands.push(cmd.as_str());
        }

        // Suite teardown
        for cmd in &suite.config.teardown {
            commands.push(cmd.as_str());
        }

        // Run all commands in a single session
        let output = driver.run(&commands).map_err(|e| {
            (
                format!("Driver error: {}", e),
                DriverOutput::default(),
                0,
            )
        })?;

        // Validate each test step's expectations against the combined output
        // Note: This is a simplified approach - a more sophisticated version
        // would track output per-step
        for (idx, step) in test.steps.iter().enumerate() {
            if let Some(expect) = &step.expect {
                if let Err(e) = validator.validate(&output, expect) {
                    return Err((format!("Step {}: {}", idx + 1, e), output, step_start_idx + idx));
                }
            }
        }

        Ok(())
    }

    /// Create a temp directory for a test.
    fn create_temp_dir(&self, test_id: &str) -> tempfile::TempDir {
        let base = self
            .config
            .temp_dir
            .as_ref()
            .map(|p| p.as_path())
            .unwrap_or_else(|| std::env::temp_dir().as_path().to_owned().leak());

        tempfile::Builder::new()
            .prefix(&format!("swebash_test_{}_", test_id))
            .tempdir_in(base)
            .unwrap_or_else(|_| tempfile::tempdir().unwrap())
    }
}

/// Number of CPUs helper (fallback for when num_cpus isn't available).
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{SuiteConfig, TestConfig};

    #[allow(dead_code)]
    fn make_test_spec(id: &str, name: &str, steps: Vec<TestStep>) -> TestSpec {
        TestSpec {
            id: id.to_string(),
            name: name.to_string(),
            config: TestConfig::default(),
            tags: Vec::new(),
            skip: false,
            skip_reason: None,
            steps,
        }
    }

    #[allow(dead_code)]
    fn make_suite(name: &str, tests: Vec<TestSpec>) -> TestSuite {
        TestSuite {
            version: 1,
            suite: name.to_string(),
            config: SuiteConfig::default(),
            tests,
        }
    }

    #[test]
    fn test_outcome_helpers() {
        let passed = TestOutcome::Passed {
            duration: Duration::from_secs(1),
        };
        assert!(passed.is_passed());
        assert!(!passed.is_failed());

        let failed = TestOutcome::Failed {
            duration: Duration::from_secs(1),
            error: "error".to_string(),
            output: None,
            step_index: None,
        };
        assert!(!failed.is_passed());
        assert!(failed.is_failed());

        let skipped = TestOutcome::Skipped { reason: None };
        assert!(skipped.is_skipped());
    }

    #[test]
    fn test_suite_result_counts() {
        let results = vec![
            TestResult {
                test_id: "1".to_string(),
                test_name: "Test 1".to_string(),
                suite_name: "suite".to_string(),
                outcome: TestOutcome::Passed {
                    duration: Duration::from_millis(100),
                },
            },
            TestResult {
                test_id: "2".to_string(),
                test_name: "Test 2".to_string(),
                suite_name: "suite".to_string(),
                outcome: TestOutcome::Failed {
                    duration: Duration::from_millis(100),
                    error: "error".to_string(),
                    output: None,
                    step_index: None,
                },
            },
            TestResult {
                test_id: "3".to_string(),
                test_name: "Test 3".to_string(),
                suite_name: "suite".to_string(),
                outcome: TestOutcome::Skipped { reason: None },
            },
        ];

        let suite_result = SuiteResult {
            suite_name: "test".to_string(),
            test_results: results,
            duration: Duration::from_secs(1),
        };

        assert_eq!(suite_result.passed_count(), 1);
        assert_eq!(suite_result.failed_count(), 1);
        assert_eq!(suite_result.skipped_count(), 1);
        assert_eq!(suite_result.total_count(), 3);
        assert!(!suite_result.all_passed());
    }
}
