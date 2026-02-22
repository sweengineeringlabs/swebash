//! Test report generation.
//!
//! Generates JSON, HTML, and terminal reports from test results.

use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::executor::{SuiteResult, TestOutcome, TestResult};

/// Report format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Terminal output with colors.
    Terminal,
    /// JSON format.
    Json,
    /// HTML format.
    Html,
    /// JUnit XML format (for CI integration).
    JUnit,
}

impl std::str::FromStr for ReportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" | "term" | "console" => Ok(ReportFormat::Terminal),
            "json" => Ok(ReportFormat::Json),
            "html" => Ok(ReportFormat::Html),
            "junit" | "xml" => Ok(ReportFormat::JUnit),
            _ => Err(format!("Unknown report format: {}", s)),
        }
    }
}

/// Full test report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Report generation timestamp.
    pub timestamp: DateTime<Utc>,

    /// Suite results.
    pub suites: Vec<SuiteReport>,

    /// Summary statistics.
    pub summary: ReportSummary,
}

/// Report for a single suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteReport {
    /// Suite name.
    pub name: String,

    /// Test results.
    pub tests: Vec<TestReport>,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Pass count.
    pub passed: usize,

    /// Fail count.
    pub failed: usize,

    /// Skip count.
    pub skipped: usize,
}

/// Report for a single test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    /// Test ID.
    pub id: String,

    /// Test name.
    pub name: String,

    /// Status: "passed", "failed", "skipped", "error".
    pub status: String,

    /// Duration in milliseconds (if applicable).
    pub duration_ms: Option<u64>,

    /// Error message (if failed).
    pub error: Option<String>,

    /// Step index where failure occurred (if applicable).
    pub failed_step: Option<usize>,

    /// Skip reason (if skipped).
    pub skip_reason: Option<String>,

    /// Stdout (truncated for large outputs).
    pub stdout: Option<String>,

    /// Stderr (truncated for large outputs).
    pub stderr: Option<String>,
}

/// Summary statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Total test count.
    pub total: usize,

    /// Passed count.
    pub passed: usize,

    /// Failed count.
    pub failed: usize,

    /// Skipped count.
    pub skipped: usize,

    /// Error count.
    pub errors: usize,

    /// Total duration in milliseconds.
    pub duration_ms: u64,

    /// Pass rate (0.0 - 1.0).
    pub pass_rate: f64,
}

/// Report generator.
pub struct Reporter {
    /// Maximum output length to include in reports.
    max_output_length: usize,
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter {
    /// Create a new reporter.
    pub fn new() -> Self {
        Self {
            max_output_length: 2000,
        }
    }

    /// Set maximum output length.
    pub fn with_max_output_length(mut self, len: usize) -> Self {
        self.max_output_length = len;
        self
    }

    /// Generate a report from suite results.
    pub fn generate(&self, results: &[SuiteResult]) -> Report {
        let suites: Vec<SuiteReport> = results.iter().map(|r| self.suite_report(r)).collect();

        let summary = self.calculate_summary(&suites);

        Report {
            timestamp: Utc::now(),
            suites,
            summary,
        }
    }

    /// Generate report for a single suite.
    fn suite_report(&self, result: &SuiteResult) -> SuiteReport {
        let tests: Vec<TestReport> = result
            .test_results
            .iter()
            .map(|r| self.test_report(r))
            .collect();

        SuiteReport {
            name: result.suite_name.clone(),
            tests,
            duration_ms: result.duration.as_millis() as u64,
            passed: result.passed_count(),
            failed: result.failed_count(),
            skipped: result.skipped_count(),
        }
    }

    /// Generate report for a single test.
    fn test_report(&self, result: &TestResult) -> TestReport {
        let (status, duration_ms, error, failed_step, skip_reason, stdout, stderr) =
            match &result.outcome {
                TestOutcome::Passed { duration } => {
                    ("passed".to_string(), Some(duration.as_millis() as u64), None, None, None, None, None)
                }
                TestOutcome::Failed {
                    duration,
                    error,
                    output,
                    step_index,
                } => {
                    let stdout = output.as_ref().map(|o| self.truncate(&o.stdout));
                    let stderr = output.as_ref().map(|o| self.truncate(&o.stderr));
                    (
                        "failed".to_string(),
                        Some(duration.as_millis() as u64),
                        Some(error.clone()),
                        *step_index,
                        None,
                        stdout,
                        stderr,
                    )
                }
                TestOutcome::Skipped { reason } => {
                    ("skipped".to_string(), None, None, None, reason.clone(), None, None)
                }
                TestOutcome::Error { error } => {
                    ("error".to_string(), None, Some(error.clone()), None, None, None, None)
                }
            };

        TestReport {
            id: result.test_id.clone(),
            name: result.test_name.clone(),
            status,
            duration_ms,
            error,
            failed_step,
            skip_reason,
            stdout,
            stderr,
        }
    }

    /// Calculate summary statistics.
    fn calculate_summary(&self, suites: &[SuiteReport]) -> ReportSummary {
        let mut total = 0;
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut errors = 0;
        let mut duration_ms = 0;

        for suite in suites {
            duration_ms += suite.duration_ms;
            for test in &suite.tests {
                total += 1;
                match test.status.as_str() {
                    "passed" => passed += 1,
                    "failed" => failed += 1,
                    "skipped" => skipped += 1,
                    "error" => errors += 1,
                    _ => {}
                }
            }
        }

        let pass_rate = if total > 0 {
            passed as f64 / total as f64
        } else {
            0.0
        };

        ReportSummary {
            total,
            passed,
            failed,
            skipped,
            errors,
            duration_ms,
            pass_rate,
        }
    }

    /// Truncate output to maximum length.
    fn truncate(&self, s: &str) -> String {
        if s.len() <= self.max_output_length {
            s.to_string()
        } else {
            format!("{}... (truncated)", &s[..self.max_output_length])
        }
    }

    /// Write report to terminal.
    pub fn write_terminal<W: Write>(&self, report: &Report, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer)?;
        writeln!(writer, "{}", "Test Results".bold())?;
        writeln!(writer, "{}", "=".repeat(60))?;
        writeln!(writer)?;

        for suite in &report.suites {
            self.write_suite_terminal(suite, writer)?;
        }

        self.write_summary_terminal(&report.summary, writer)?;

        Ok(())
    }

    /// Write suite results to terminal.
    fn write_suite_terminal<W: Write>(
        &self,
        suite: &SuiteReport,
        writer: &mut W,
    ) -> std::io::Result<()> {
        writeln!(writer, "{} {}", "Suite:".bold(), suite.name)?;
        writeln!(writer, "{}", "-".repeat(60))?;

        for test in &suite.tests {
            let status_icon = match test.status.as_str() {
                "passed" => "PASS".green(),
                "failed" => "FAIL".red(),
                "skipped" => "SKIP".yellow(),
                "error" => "ERR ".red().bold(),
                _ => "????".normal(),
            };

            let duration_str = test
                .duration_ms
                .map(|d| format!(" ({:.2}s)", d as f64 / 1000.0))
                .unwrap_or_default();

            writeln!(
                writer,
                "  [{}] {}: {}{}",
                status_icon,
                test.id,
                test.name,
                duration_str.dimmed()
            )?;

            if let Some(error) = &test.error {
                writeln!(writer, "         {}: {}", "Error".red(), error)?;
            }

            if let Some(reason) = &test.skip_reason {
                writeln!(writer, "         {}: {}", "Reason".yellow(), reason)?;
            }
        }

        writeln!(writer)?;
        Ok(())
    }

    /// Write summary to terminal.
    fn write_summary_terminal<W: Write>(
        &self,
        summary: &ReportSummary,
        writer: &mut W,
    ) -> std::io::Result<()> {
        writeln!(writer, "{}", "=".repeat(60))?;
        writeln!(writer, "{}", "Summary".bold())?;
        writeln!(writer)?;

        writeln!(
            writer,
            "  Total:   {} tests",
            summary.total.to_string().bold()
        )?;
        writeln!(
            writer,
            "  Passed:  {}",
            summary.passed.to_string().green()
        )?;
        writeln!(
            writer,
            "  Failed:  {}",
            if summary.failed > 0 {
                summary.failed.to_string().red()
            } else {
                summary.failed.to_string().normal()
            }
        )?;
        writeln!(
            writer,
            "  Skipped: {}",
            if summary.skipped > 0 {
                summary.skipped.to_string().yellow()
            } else {
                summary.skipped.to_string().normal()
            }
        )?;
        if summary.errors > 0 {
            writeln!(
                writer,
                "  Errors:  {}",
                summary.errors.to_string().red().bold()
            )?;
        }

        writeln!(writer)?;
        writeln!(
            writer,
            "  Duration: {:.2}s",
            summary.duration_ms as f64 / 1000.0
        )?;
        writeln!(
            writer,
            "  Pass rate: {:.1}%",
            summary.pass_rate * 100.0
        )?;

        writeln!(writer)?;

        if summary.failed == 0 && summary.errors == 0 {
            writeln!(writer, "{}", "All tests passed!".green().bold())?;
        } else {
            writeln!(
                writer,
                "{}",
                format!("{} test(s) failed", summary.failed + summary.errors)
                    .red()
                    .bold()
            )?;
        }

        writeln!(writer)?;
        Ok(())
    }

    /// Write report as JSON.
    pub fn write_json<W: Write>(&self, report: &Report, writer: &mut W) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(report)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(writer, "{}", json)
    }

    /// Write report as HTML.
    pub fn write_html<W: Write>(&self, report: &Report, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "<!DOCTYPE html>")?;
        writeln!(writer, "<html lang=\"en\">")?;
        writeln!(writer, "<head>")?;
        writeln!(writer, "  <meta charset=\"UTF-8\">")?;
        writeln!(writer, "  <title>swebash Test Report</title>")?;
        writeln!(writer, "  <style>")?;
        writeln!(writer, "    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 2em; }}")?;
        writeln!(writer, "    .passed {{ color: #22c55e; }}")?;
        writeln!(writer, "    .failed {{ color: #ef4444; }}")?;
        writeln!(writer, "    .skipped {{ color: #eab308; }}")?;
        writeln!(writer, "    .error {{ color: #ef4444; font-weight: bold; }}")?;
        writeln!(writer, "    table {{ border-collapse: collapse; width: 100%; }}")?;
        writeln!(writer, "    th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}")?;
        writeln!(writer, "    th {{ background-color: #f3f4f6; }}")?;
        writeln!(writer, "    .summary {{ background-color: #f9fafb; padding: 1em; margin: 1em 0; border-radius: 8px; }}")?;
        writeln!(writer, "    pre {{ background-color: #1f2937; color: #f9fafb; padding: 1em; overflow-x: auto; border-radius: 4px; }}")?;
        writeln!(writer, "  </style>")?;
        writeln!(writer, "</head>")?;
        writeln!(writer, "<body>")?;

        writeln!(writer, "<h1>swebash Test Report</h1>")?;
        writeln!(
            writer,
            "<p>Generated: {}</p>",
            report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        )?;

        // Summary
        writeln!(writer, "<div class=\"summary\">")?;
        writeln!(writer, "<h2>Summary</h2>")?;
        writeln!(
            writer,
            "<p>Total: {} | <span class=\"passed\">Passed: {}</span> | <span class=\"failed\">Failed: {}</span> | <span class=\"skipped\">Skipped: {}</span></p>",
            report.summary.total,
            report.summary.passed,
            report.summary.failed,
            report.summary.skipped
        )?;
        writeln!(
            writer,
            "<p>Duration: {:.2}s | Pass rate: {:.1}%</p>",
            report.summary.duration_ms as f64 / 1000.0,
            report.summary.pass_rate * 100.0
        )?;
        writeln!(writer, "</div>")?;

        // Suites
        for suite in &report.suites {
            writeln!(writer, "<h2>Suite: {}</h2>", suite.name)?;
            writeln!(writer, "<table>")?;
            writeln!(
                writer,
                "<tr><th>Status</th><th>ID</th><th>Name</th><th>Duration</th><th>Details</th></tr>"
            )?;

            for test in &suite.tests {
                let status_class = &test.status;
                let duration = test
                    .duration_ms
                    .map(|d| format!("{:.2}s", d as f64 / 1000.0))
                    .unwrap_or_else(|| "-".to_string());

                let details = test
                    .error
                    .as_ref()
                    .or(test.skip_reason.as_ref())
                    .cloned()
                    .unwrap_or_default();

                writeln!(
                    writer,
                    "<tr><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    status_class,
                    test.status.to_uppercase(),
                    html_escape(&test.id),
                    html_escape(&test.name),
                    duration,
                    html_escape(&details)
                )?;
            }

            writeln!(writer, "</table>")?;
        }

        writeln!(writer, "</body>")?;
        writeln!(writer, "</html>")?;

        Ok(())
    }

    /// Write report in specified format.
    pub fn write<W: Write>(
        &self,
        report: &Report,
        format: ReportFormat,
        writer: &mut W,
    ) -> std::io::Result<()> {
        match format {
            ReportFormat::Terminal => self.write_terminal(report, writer),
            ReportFormat::Json => self.write_json(report, writer),
            ReportFormat::Html => self.write_html(report, writer),
            ReportFormat::JUnit => self.write_junit(report, writer),
        }
    }

    /// Write report as JUnit XML.
    pub fn write_junit<W: Write>(&self, report: &Report, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        writeln!(
            writer,
            "<testsuites tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\" time=\"{:.3}\">",
            report.summary.total,
            report.summary.failed,
            report.summary.errors,
            report.summary.skipped,
            report.summary.duration_ms as f64 / 1000.0
        )?;

        for suite in &report.suites {
            writeln!(
                writer,
                "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" skipped=\"{}\" time=\"{:.3}\">",
                xml_escape(&suite.name),
                suite.tests.len(),
                suite.failed,
                suite.skipped,
                suite.duration_ms as f64 / 1000.0
            )?;

            for test in &suite.tests {
                let time = test.duration_ms.unwrap_or(0) as f64 / 1000.0;
                writeln!(
                    writer,
                    "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\">",
                    xml_escape(&test.name),
                    xml_escape(&test.id),
                    time
                )?;

                match test.status.as_str() {
                    "failed" => {
                        if let Some(error) = &test.error {
                            writeln!(
                                writer,
                                "      <failure message=\"{}\"/>",
                                xml_escape(error)
                            )?;
                        }
                    }
                    "error" => {
                        if let Some(error) = &test.error {
                            writeln!(
                                writer,
                                "      <error message=\"{}\"/>",
                                xml_escape(error)
                            )?;
                        }
                    }
                    "skipped" => {
                        let reason = test.skip_reason.as_deref().unwrap_or("");
                        writeln!(
                            writer,
                            "      <skipped message=\"{}\"/>",
                            xml_escape(reason)
                        )?;
                    }
                    _ => {}
                }

                if let Some(stdout) = &test.stdout {
                    writeln!(
                        writer,
                        "      <system-out><![CDATA[{}]]></system-out>",
                        stdout
                    )?;
                }

                if let Some(stderr) = &test.stderr {
                    writeln!(
                        writer,
                        "      <system-err><![CDATA[{}]]></system-err>",
                        stderr
                    )?;
                }

                writeln!(writer, "    </testcase>")?;
            }

            writeln!(writer, "  </testsuite>")?;
        }

        writeln!(writer, "</testsuites>")?;

        Ok(())
    }

    /// Save report to file.
    pub fn save(&self, report: &Report, format: ReportFormat, path: &Path) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        self.write(report, format, &mut file)
    }
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Escape XML special characters.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_test_result(id: &str, name: &str, outcome: TestOutcome) -> TestResult {
        TestResult {
            test_id: id.to_string(),
            test_name: name.to_string(),
            suite_name: "test_suite".to_string(),
            outcome,
        }
    }

    fn make_suite_result(name: &str, results: Vec<TestResult>) -> SuiteResult {
        SuiteResult {
            suite_name: name.to_string(),
            test_results: results,
            duration: Duration::from_secs(1),
        }
    }

    #[test]
    fn test_report_generation() {
        let reporter = Reporter::new();
        let results = vec![make_suite_result(
            "test_suite",
            vec![
                make_test_result(
                    "test_1",
                    "Test 1",
                    TestOutcome::Passed {
                        duration: Duration::from_millis(100),
                    },
                ),
                make_test_result(
                    "test_2",
                    "Test 2",
                    TestOutcome::Failed {
                        duration: Duration::from_millis(200),
                        error: "assertion failed".to_string(),
                        output: None,
                        step_index: Some(0),
                    },
                ),
            ],
        )];

        let report = reporter.generate(&results);

        assert_eq!(report.summary.total, 2);
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 1);
    }

    #[test]
    fn test_json_output() {
        let reporter = Reporter::new();
        let results = vec![make_suite_result(
            "test_suite",
            vec![make_test_result(
                "test_1",
                "Test 1",
                TestOutcome::Passed {
                    duration: Duration::from_millis(100),
                },
            )],
        )];

        let report = reporter.generate(&results);
        let mut output = Vec::new();
        reporter.write_json(&report, &mut output).unwrap();

        let json_str = String::from_utf8(output).unwrap();
        assert!(json_str.contains("\"passed\": 1"));
    }

    #[test]
    fn test_format_parsing() {
        assert_eq!("json".parse::<ReportFormat>().unwrap(), ReportFormat::Json);
        assert_eq!("html".parse::<ReportFormat>().unwrap(), ReportFormat::Html);
        assert_eq!(
            "terminal".parse::<ReportFormat>().unwrap(),
            ReportFormat::Terminal
        );
        assert_eq!(
            "junit".parse::<ReportFormat>().unwrap(),
            ReportFormat::JUnit
        );
    }
}
