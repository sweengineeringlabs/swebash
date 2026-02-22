//! swebash-autotest: Automated interactive test runner for swebash.
//!
//! Usage:
//!   swebash-autotest [OPTIONS] [SUITE_PATH]...
//!
//! Examples:
//!   swebash-autotest                           # Run all tests in tests/suites/
//!   swebash-autotest --suite shell_basics      # Run specific suite
//!   swebash-autotest --tags smoke              # Run tests with 'smoke' tag
//!   swebash-autotest --parallel 4              # Use 4 parallel workers
//!   swebash-autotest --format json > report.json

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use swebash_autotest::executor::{Executor, ExecutorConfig};
use swebash_autotest::report::{ReportFormat, Reporter};
use swebash_autotest::spec::TestSuite;

/// Automated interactive test runner for swebash.
#[derive(Parser, Debug)]
#[command(name = "swebash-autotest")]
#[command(version, about, long_about = None)]
struct Args {
    /// Test suite files or directories to run.
    /// If not specified, runs all suites in tests/suites/
    #[arg(value_name = "SUITE_PATH")]
    suites: Vec<PathBuf>,

    /// Run only the specified suite (by name).
    #[arg(short, long)]
    suite: Option<String>,

    /// Run tests with these tags (comma-separated).
    #[arg(short, long, value_delimiter = ',')]
    tags: Vec<String>,

    /// Exclude tests with these tags (comma-separated).
    #[arg(long, value_delimiter = ',')]
    exclude_tags: Vec<String>,

    /// Number of parallel workers.
    #[arg(short, long, default_value = "4")]
    parallel: usize,

    /// Disable parallel execution.
    #[arg(long)]
    no_parallel: bool,

    /// Output format: terminal, json, html, junit.
    #[arg(short, long, default_value = "terminal")]
    format: String,

    /// Output file (defaults to stdout).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Verbose output (show all command output).
    #[arg(short, long)]
    verbose: bool,

    /// Stop on first failure.
    #[arg(long)]
    fail_fast: bool,

    /// Path to the swebash binary.
    #[arg(long)]
    binary: Option<PathBuf>,

    /// List available suites without running.
    #[arg(long)]
    list: bool,

    /// Dry run (parse suites but don't execute).
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Handle --list
    if args.list {
        return list_suites(&args);
    }

    // Parse format
    let format: ReportFormat = match args.format.parse() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {}", "Error".red(), e);
            return ExitCode::FAILURE;
        }
    };

    // Find suite files
    let suite_files = match find_suite_files(&args) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{}: {}", "Error".red(), e);
            return ExitCode::FAILURE;
        }
    };

    if suite_files.is_empty() {
        eprintln!("{}: No test suites found", "Warning".yellow());
        return ExitCode::SUCCESS;
    }

    // Parse suites
    let suites = match parse_suites(&suite_files) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", "Error".red(), e);
            return ExitCode::FAILURE;
        }
    };

    // Handle --dry-run
    if args.dry_run {
        return dry_run(&suites);
    }

    // Filter by suite name if specified
    let suites: Vec<_> = if let Some(ref name) = args.suite {
        suites.into_iter().filter(|s| s.suite == *name).collect()
    } else {
        suites
    };

    if suites.is_empty() {
        if let Some(name) = &args.suite {
            eprintln!("{}: Suite '{}' not found", "Error".red(), name);
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    // Create executor
    let config = ExecutorConfig {
        binary_path: args.binary,
        temp_dir: None,
        include_tags: args.tags,
        exclude_tags: args.exclude_tags,
        parallel_workers: args.parallel,
        parallel: !args.no_parallel,
        verbose: args.verbose,
        continue_on_failure: !args.fail_fast,
    };

    let executor = Executor::new(config);

    // Show progress bar for terminal output
    let show_progress = format == ReportFormat::Terminal && !args.verbose;
    let progress = if show_progress {
        let pb = ProgressBar::new(suites.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Execute suites
    let mut results = Vec::new();
    for suite in &suites {
        if let Some(ref pb) = progress {
            pb.set_message(format!("Running {}", suite.suite));
        }

        let result = executor.execute_suite(suite);
        results.push(result);

        if let Some(ref pb) = progress {
            pb.inc(1);
        }
    }

    if let Some(pb) = progress {
        pb.finish_with_message("Done");
    }

    // Generate report
    let reporter = Reporter::new();
    let report = reporter.generate(&results);

    // Write output
    let write_result = if let Some(output_path) = &args.output {
        reporter.save(&report, format, output_path)
    } else {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        reporter.write(&report, format, &mut handle)
    };

    if let Err(e) = write_result {
        eprintln!("{}: Failed to write report: {}", "Error".red(), e);
        return ExitCode::FAILURE;
    }

    // Exit code based on results
    if report.summary.failed == 0 && report.summary.errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Find test suite files based on arguments.
fn find_suite_files(args: &Args) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    if args.suites.is_empty() {
        // Default: look in tests/suites/
        let default_dir = PathBuf::from("tests/suites");
        if default_dir.exists() {
            collect_yaml_files(&default_dir, &mut files)?;
        }
    } else {
        for path in &args.suites {
            if path.is_file() {
                files.push(path.clone());
            } else if path.is_dir() {
                collect_yaml_files(path, &mut files)?;
            } else {
                return Err(format!("Path not found: {}", path.display()));
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Collect YAML files from a directory.
fn collect_yaml_files(dir: &PathBuf, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    files.push(path);
                }
            }
        }
    }

    Ok(())
}

/// Parse suite files into TestSuite objects.
fn parse_suites(files: &[PathBuf]) -> Result<Vec<TestSuite>, String> {
    let mut suites = Vec::new();

    for file in files {
        let suite = TestSuite::from_file(file)?;
        suites.push(suite);
    }

    Ok(suites)
}

/// List available suites.
fn list_suites(args: &Args) -> ExitCode {
    let suite_files = match find_suite_files(args) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{}: {}", "Error".red(), e);
            return ExitCode::FAILURE;
        }
    };

    println!("{}", "Available test suites:".bold());
    println!();

    for file in &suite_files {
        match TestSuite::from_file(file) {
            Ok(suite) => {
                println!(
                    "  {} ({} tests)",
                    suite.suite.green(),
                    suite.tests.len()
                );
                println!("    File: {}", file.display());
                if !suite.config.tags.is_empty() {
                    println!("    Tags: {}", suite.config.tags.join(", "));
                }
            }
            Err(e) => {
                eprintln!(
                    "  {} (parse error: {})",
                    file.display().to_string().red(),
                    e
                );
            }
        }
    }

    println!();
    ExitCode::SUCCESS
}

/// Dry run: parse and validate suites without executing.
fn dry_run(suites: &[TestSuite]) -> ExitCode {
    println!("{}", "Dry run - validating test suites:".bold());
    println!();

    let mut total_tests = 0;
    let mut errors = 0;

    for suite in suites {
        println!("  Suite: {}", suite.suite.green());
        println!("    Tests: {}", suite.tests.len());
        println!("    Timeout: {}ms", suite.config.timeout_ms);
        println!("    Parallel: {}", suite.config.parallel);

        for test in &suite.tests {
            total_tests += 1;
            print!("    - {}: ", test.id);

            if test.skip {
                println!("{}", "SKIP".yellow());
            } else if test.steps.is_empty() {
                println!("{}", "ERROR (no steps)".red());
                errors += 1;
            } else {
                println!(
                    "{} ({} steps)",
                    "OK".green(),
                    test.steps.len()
                );
            }
        }

        println!();
    }

    println!("{}", "Summary:".bold());
    println!("  Suites: {}", suites.len());
    println!("  Tests: {}", total_tests);

    if errors > 0 {
        println!("  {}: {}", "Errors".red(), errors);
        ExitCode::FAILURE
    } else {
        println!("  {}", "All suites valid".green());
        ExitCode::SUCCESS
    }
}
