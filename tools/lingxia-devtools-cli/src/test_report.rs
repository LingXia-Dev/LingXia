//! `lxdev test report [DIR|latest]`: an earlier run's summary, failures and
//! Rerun lines again, from its report.json — no session needed.

use crate::test::{ReportFormat, ReportOptions, failed_at, failure_records, rerun_with_id};
use anyhow::{Context, Result, anyhow};
use owo_colors::OwoColorize;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::Path;

pub fn execute(options: &ReportOptions) -> Result<()> {
    let report_path = crate::test::resolve_report_path(&options.run)?;
    let run_dir = report_path.parent().unwrap_or(Path::new("."));
    match options.format {
        ReportFormat::Junit => {
            let junit = run_dir.join("junit.xml");
            let text = std::fs::read_to_string(&junit)
                .with_context(|| format!("{} has no junit.xml", run_dir.display()))?;
            print!("{text}");
            Ok(())
        }
        ReportFormat::Json => {
            let report = read(&report_path)?;
            let value = if options.failures {
                failure_records(report["cases"].as_array().map_or(&[][..], Vec::as_slice))
            } else {
                report
            };
            println!("{}", serde_json::to_string_pretty(&value)?);
            Ok(())
        }
        ReportFormat::Text => {
            let report = read(&report_path)?;
            eprint!("{}", render(&report, run_dir, options.failures));
            Ok(())
        }
    }
}

fn read(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| anyhow!("{} is not a report: {err}", path.display()))
}

fn count(report: &Value, key: &str) -> u64 {
    report[key].as_u64().unwrap_or(0)
}

/// The text a finished run printed: counts, then each failure with where it
/// failed and its Rerun line, then the report files.
pub fn render(report: &Value, run_dir: &Path, failures_only: bool) -> String {
    let mut out = String::new();
    let cases = report["cases"].as_array().map_or(&[][..], Vec::as_slice);
    if !failures_only {
        let started = report["meta"]["started_at"].as_str().unwrap_or("-");
        let _ = writeln!(out, "{} {}", "run".cyan(), run_dir.display());
        let _ = writeln!(out, "  started {started}");
        let _ = writeln!(
            out,
            "{} passed, {} failed, {} skipped, {} timeout, {} xfail, {} xpass ({} cases, {:.1}s)",
            count(report, "passed"),
            count(report, "failed"),
            count(report, "skipped"),
            count(report, "timeout"),
            count(report, "xfail"),
            count(report, "xpass"),
            report["total"].as_u64().unwrap_or(cases.len() as u64),
            count(report, "duration_ms") as f64 / 1000.0,
        );
        if report["partial"] == json!(true) {
            let _ = writeln!(out, "Incomplete run; not every selected spec ran.");
        }
    }
    let rerun = report["meta"]["rerun"].as_str();
    let mut failed = 0usize;
    for case in cases {
        let status = case["status"].as_str().unwrap_or("");
        if !matches!(status, "failed" | "timeout" | "xpass") {
            continue;
        }
        failed += 1;
        let error = &case["error"];
        let name = case["full_name"]
            .as_str()
            .or_else(|| case["title"].as_str())
            .unwrap_or("?");
        let message = error["message"].as_str().unwrap_or(status);
        let _ = writeln!(out, "\n{} [{status}]: {message}", name.red());
        if let Some(detail) = error.as_object() {
            if let Some(line) = failed_at(detail) {
                let _ = writeln!(out, "  {line}");
            }
            for field in ["code", "phase", "step", "location", "expected", "actual"] {
                if let Some(value) = detail.get(field).and_then(Value::as_str) {
                    let _ = writeln!(out, "  {field}: {value}");
                }
            }
        }
        if let (Some(file), Some(line)) = (case["file"].as_str(), case["line"].as_u64()) {
            let _ = writeln!(out, "  at {file}:{line}");
        }
        if let (Some(rerun), Some(id)) = (rerun, case["id"].as_str()) {
            let _ = writeln!(out, "  Rerun: {}", rerun_with_id(rerun, id));
        }
    }
    if failed == 0 && failures_only {
        let _ = writeln!(out, "No failed specs.");
    }
    if !failures_only {
        for name in ["report.html", "report.json", "junit.xml"] {
            let path = run_dir.join(name);
            if path.is_file() {
                let _ = writeln!(out, "{} {}", format!("{name:>11}").cyan(), path.display());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Value {
        json!({
            "total": 3, "passed": 1, "failed": 1, "skipped": 0, "timeout": 1,
            "xfail": 0, "xpass": 0, "duration_ms": 4200, "partial": false,
            "meta": { "started_at": "2026-09-25T10:00:00Z", "rerun": "lxdev test 'tests/'" },
            "cases": [
                { "id": "A-1", "full_name": "home loads", "status": "passed" },
                { "id": "B-2", "full_name": "cart totals", "status": "failed",
                  "file": "tests/cart.test.ts", "line": 12,
                  "error": { "message": "expected 3, got 2", "code": "E_ASSERT",
                             "expected": "3", "actual": "2" } },
                { "id": "C-3", "full_name": "slow page", "status": "timeout",
                  "error": { "message": "spec timed out after 30000ms" } }
            ]
        })
    }

    #[test]
    fn a_report_renders_its_summary_failures_and_rerun_lines() {
        let text = render(&report(), Path::new("/r/run-1"), false);
        assert!(
            text.contains("1 passed, 1 failed, 0 skipped, 1 timeout"),
            "{text}"
        );
        assert!(text.contains("(3 cases, 4.2s)"), "{text}");
        assert!(text.contains("expected 3, got 2"), "{text}");
        assert!(text.contains("code: E_ASSERT"), "{text}");
        assert!(text.contains("at tests/cart.test.ts:12"), "{text}");
        assert!(
            text.contains("Rerun: lxdev test 'tests/' --id 'B-2'"),
            "{text}"
        );
        assert!(
            text.contains("Rerun: lxdev test 'tests/' --id 'C-3'"),
            "{text}"
        );
        assert!(!text.contains("--id 'A-1'"), "{text}");
    }

    #[test]
    fn failures_only_skips_the_summary() {
        let text = render(&report(), Path::new("/r"), true);
        assert!(!text.contains("passed,"), "{text}");
        assert!(text.contains("cart totals"), "{text}");
        let clean = json!({ "cases": [{ "id": "A", "status": "passed" }] });
        assert!(render(&clean, Path::new("/r"), true).contains("No failed specs."));
        // A report without a recorded rerun command still renders.
        let mut legacy = report();
        legacy["meta"] = json!({});
        assert!(!render(&legacy, Path::new("/r"), false).contains("Rerun"));
    }

    #[test]
    fn json_and_junit_come_from_the_run_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("report.json"), report().to_string()).unwrap();
        let options = |format, failures| ReportOptions {
            run: dir.path().to_path_buf(),
            failures,
            format,
        };
        execute(&options(ReportFormat::Json, true)).unwrap();
        execute(&options(ReportFormat::Text, false)).unwrap();
        let error = execute(&options(ReportFormat::Junit, false)).unwrap_err();
        assert!(error.to_string().contains("no junit.xml"), "{error}");
        std::fs::write(dir.path().join("junit.xml"), "<testsuites/>").unwrap();
        execute(&options(ReportFormat::Junit, false)).unwrap();
        let failures = failure_records(report()["cases"].as_array().unwrap());
        assert_eq!(failures.as_array().map(Vec::len), Some(2));
    }
}
