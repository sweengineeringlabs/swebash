#!/usr/bin/env python3
"""
Convert manual test specifications from Markdown tables to YAML test specs.

Usage:
    python scripts/convert_manual_tests.py

This script reads manual test files from docs/5-testing/manual_*.md and
generates corresponding YAML test specifications in tests/suites/.

Input format (Markdown table):
    | ID | Category | Test Name | Steps | Expected Result | Status |
    |----|----------|-----------|-------|-----------------|--------|
    | 1  | Shell    | Echo test | 1. Run `echo hello` | Shows "hello" | - |

Output format (YAML):
    tests:
      - id: shell_1
        name: "Echo test"
        steps:
          - command: "echo hello"
            expect:
              contains: "hello"
"""

import argparse
import re
import sys
from pathlib import Path
from typing import Optional


def parse_markdown_table(content: str) -> list[dict]:
    """Parse a Markdown table into a list of dictionaries."""
    lines = content.strip().split("\n")
    tests = []
    headers = []

    for line in lines:
        line = line.strip()
        if not line or not line.startswith("|"):
            continue

        # Skip separator lines
        if re.match(r"^\|[\s\-:|]+\|$", line):
            continue

        # Parse cells
        cells = [cell.strip() for cell in line.split("|")[1:-1]]

        if not headers:
            # First row is headers
            headers = [h.lower().replace(" ", "_") for h in cells]
        else:
            # Data row
            row = dict(zip(headers, cells))
            if row.get("id"):
                tests.append(row)

    return tests


def extract_commands(steps_text: str) -> list[str]:
    """Extract shell commands from step descriptions."""
    commands = []

    # Match patterns like:
    # - `command`
    # - Run `command`
    # - Enter `command`
    # - Type `command`
    # - 1. Run `command`
    patterns = [
        r"`([^`]+)`",  # Anything in backticks
    ]

    for pattern in patterns:
        matches = re.findall(pattern, steps_text)
        for match in matches:
            # Filter out non-command strings
            if not match.startswith("$") and len(match) > 0:
                commands.append(match)

    return commands


def extract_expectations(expected_text: str) -> dict:
    """Convert expected result text to validation rules."""
    expectations = {}
    text_lower = expected_text.lower()

    # Look for specific patterns
    if "error" in text_lower:
        # Error expectations go to stderr
        expectations["stderr"] = {"contains": "error"}

    # Extract quoted strings as contains checks
    quoted = re.findall(r'"([^"]+)"', expected_text)
    if quoted:
        if len(quoted) == 1:
            expectations["contains"] = quoted[0]
        else:
            expectations["all"] = [{"contains": q} for q in quoted]

    # Look for "shows" or "displays" patterns
    shows_match = re.search(r"(?:shows?|displays?|outputs?|prints?)\s+(.+)", expected_text, re.IGNORECASE)
    if shows_match and "contains" not in expectations:
        text = shows_match.group(1).strip()
        # Clean up the text
        text = re.sub(r"^['\"]|['\"]$", "", text)
        if text:
            expectations["contains"] = text

    # Look for "not" patterns
    not_match = re.search(r"(?:should\s+)?not\s+(?:show|display|contain)\s+(.+)", expected_text, re.IGNORECASE)
    if not_match:
        text = not_match.group(1).strip()
        text = re.sub(r"^['\"]|['\"]$", "", text)
        if text:
            expectations["not_contains"] = text

    return expectations if expectations else None


def generate_test_id(category: str, test_id: str, name: str) -> str:
    """Generate a unique test ID."""
    # Clean category
    cat = re.sub(r"[^a-z0-9]", "_", category.lower())

    # Use numeric ID if available, otherwise derive from name
    if test_id and test_id.isdigit():
        return f"{cat}_{test_id}"

    # Derive from name
    name_slug = re.sub(r"[^a-z0-9]", "_", name.lower())
    name_slug = re.sub(r"_+", "_", name_slug)[:30]
    return f"{cat}_{name_slug}"


def infer_tags(category: str, name: str) -> list[str]:
    """Infer test tags from category and name."""
    tags = []

    cat_lower = category.lower()
    name_lower = name.lower()

    # Category-based tags
    if "ai" in cat_lower:
        tags.append("ai")
    if "git" in cat_lower:
        tags.append("git")
    if "rag" in cat_lower:
        tags.extend(["ai", "rag"])
    if "workspace" in cat_lower or "sandbox" in cat_lower:
        tags.append("workspace")
    if "shell" in cat_lower or "basic" in cat_lower:
        tags.append("shell")

    # Name-based tags
    if "smoke" in name_lower or "basic" in name_lower:
        tags.append("smoke")
    if "flaky" in name_lower or "intermittent" in name_lower:
        tags.append("flaky")

    return list(set(tags))


def convert_test(row: dict, category: str) -> dict:
    """Convert a single test row to YAML spec format."""
    test_id = generate_test_id(
        category,
        row.get("id", ""),
        row.get("test_name", row.get("name", "unknown"))
    )

    name = row.get("test_name", row.get("name", "Untitled test"))
    steps_text = row.get("steps", "")
    expected_text = row.get("expected_result", row.get("expected", ""))
    status = row.get("status", "")

    # Extract commands
    commands = extract_commands(steps_text)

    # Build steps
    steps = []
    for i, cmd in enumerate(commands):
        step = {"command": cmd}

        # Add expectation to last command
        if i == len(commands) - 1:
            expect = extract_expectations(expected_text)
            if expect:
                step["expect"] = expect

        steps.append(step)

    # If no commands found, create a placeholder
    if not steps:
        steps = [{"command": "# TODO: Add commands", "description": steps_text}]

    test = {
        "id": test_id,
        "name": name,
        "steps": steps,
    }

    # Add tags
    tags = infer_tags(category, name)
    if tags:
        test["tags"] = tags

    # Mark as skipped if status indicates
    if status and status.lower() in ["skip", "skipped", "blocked", "todo"]:
        test["skip"] = True
        test["skip_reason"] = f"Imported from manual tests with status: {status}"

    return test


def generate_suite_yaml(suite_name: str, tests: list[dict], timeout_ms: int = 30000) -> str:
    """Generate YAML content for a test suite."""
    import yaml

    # Determine suite-level tags
    all_tags = set()
    for test in tests:
        all_tags.update(test.get("tags", []))

    suite = {
        "version": 1,
        "suite": suite_name,
        "config": {
            "timeout_ms": timeout_ms,
            "parallel": True,
        },
        "tests": tests,
    }

    if all_tags:
        suite["config"]["tags"] = list(all_tags)[:3]  # Top 3 common tags

    # Custom YAML dumper for better formatting
    class IndentDumper(yaml.SafeDumper):
        pass

    def str_representer(dumper, data):
        if "\n" in data:
            return dumper.represent_scalar("tag:yaml.org,2002:str", data, style="|")
        return dumper.represent_scalar("tag:yaml.org,2002:str", data)

    IndentDumper.add_representer(str, str_representer)

    return yaml.dump(suite, Dumper=IndentDumper, default_flow_style=False, sort_keys=False, allow_unicode=True)


def process_markdown_file(input_path: Path, output_dir: Path) -> Optional[Path]:
    """Process a single Markdown file and generate YAML output."""
    print(f"Processing: {input_path}")

    content = input_path.read_text(encoding="utf-8")

    # Extract suite name from filename
    # manual_shell_tests.md -> shell
    # manual_ai_tests.md -> ai
    stem = input_path.stem
    if stem.startswith("manual_"):
        stem = stem[7:]
    if stem.endswith("_tests"):
        stem = stem[:-6]

    suite_name = stem or "imported"

    # Find tables in the Markdown
    # Split by headers to find different sections
    sections = re.split(r"^##?\s+(.+)$", content, flags=re.MULTILINE)

    all_tests = []
    current_category = suite_name

    for i, section in enumerate(sections):
        # Check if this is a header (odd indices after split)
        if i % 2 == 1:
            current_category = section.strip()
            continue

        # Parse tables in this section
        tests = parse_markdown_table(section)

        for row in tests:
            test = convert_test(row, current_category)
            all_tests.append(test)

    if not all_tests:
        print(f"  No tests found in {input_path.name}")
        return None

    # Determine timeout based on content
    timeout_ms = 30000
    if "ai" in suite_name.lower() or "rag" in suite_name.lower():
        timeout_ms = 60000

    # Generate YAML
    yaml_content = generate_suite_yaml(suite_name, all_tests, timeout_ms)

    # Write output
    output_path = output_dir / f"{suite_name}.yaml"
    output_path.write_text(yaml_content, encoding="utf-8")

    print(f"  Generated: {output_path} ({len(all_tests)} tests)")
    return output_path


def main():
    parser = argparse.ArgumentParser(
        description="Convert manual test Markdown files to YAML test specs"
    )
    parser.add_argument(
        "--input-dir",
        type=Path,
        default=Path("docs/5-testing"),
        help="Directory containing manual test Markdown files",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("tests/suites"),
        help="Directory to write generated YAML files",
    )
    parser.add_argument(
        "--pattern",
        default="manual_*.md",
        help="Glob pattern for input files",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Parse files but don't write output",
    )

    args = parser.parse_args()

    # Ensure output directory exists
    args.output_dir.mkdir(parents=True, exist_ok=True)

    # Find input files
    input_files = sorted(args.input_dir.glob(args.pattern))

    if not input_files:
        print(f"No files matching {args.pattern} found in {args.input_dir}")
        return 1

    print(f"Found {len(input_files)} input file(s)")
    print()

    generated = 0
    for input_file in input_files:
        if args.dry_run:
            content = input_file.read_text(encoding="utf-8")
            tests = parse_markdown_table(content)
            print(f"{input_file.name}: {len(tests)} tests found")
        else:
            output = process_markdown_file(input_file, args.output_dir)
            if output:
                generated += 1

    print()
    print(f"Generated {generated} suite file(s) in {args.output_dir}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
