//! Integration tests for the `serde-saphyr` binary (src/main.rs).
//!
//! These tests invoke the compiled binary via `cargo run` / `Command` and check
//! exit codes and output for various CLI scenarios.
//!
//! These tests are disabled under Miri because they spawn external processes,
//! which Miri does not support.
//!
//! These tests are also disabled for WASI because they invoke the compiled binary
//! via `Command`, which is typically not supported in WASI environments.
#![cfg(all(not(miri), not(target_os = "wasi")))]


use std::io::Write;
use std::process::Command;

/// Helper: run the binary with the given args and return (stdout, stderr, exit_code).
fn run_binary(args: &[&str]) -> (String, String, i32) {
    let bin = env!("CARGO_BIN_EXE_serde-saphyr");
    let output = Command::new(bin)
        .args(args)
        .output()
        .expect("failed to execute binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn help_flag_prints_usage_and_exits_zero() {
    let (stdout, _stderr, code) = run_binary(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage:"), "stdout: {stdout}");
}

#[test]
fn h_flag_prints_usage_and_exits_zero() {
    let (stdout, _stderr, code) = run_binary(&["-h"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage:"), "stdout: {stdout}");
}

#[test]
fn no_args_prints_usage_to_stderr_and_exits_one() {
    let (_stdout, stderr, code) = run_binary(&[]);
    assert_eq!(code, 1);
    assert!(stderr.contains("Usage:"), "stderr: {stderr}");
}

#[test]
fn unknown_option_prints_error_and_exits_one() {
    let (_stdout, stderr, code) = run_binary(&["--bogus"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("Unknown option"), "stderr: {stderr}");
}

#[test]
fn extra_argument_prints_error_and_exits_one() {
    let (_stdout, stderr, code) = run_binary(&["file1.yaml", "file2.yaml"]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("Unexpected extra argument"),
        "stderr: {stderr}"
    );
}

#[test]
fn missing_file_prints_error_and_exits_two() {
    let (_stdout, stderr, code) = run_binary(&["nonexistent_file_12345.yaml"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("Failed to read"), "stderr: {stderr}");
}

#[test]
fn valid_yaml_file_exits_zero() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    writeln!(tmp, "key: value").unwrap();
    let path = tmp.path().to_str().unwrap();

    let (stdout, _stderr, code) = run_binary(&[path]);
    assert_eq!(code, 0, "stderr: {_stderr}");
    assert!(stdout.contains("Budget report"), "stdout: {stdout}");
}

#[test]
fn valid_yaml_file_plain_mode_exits_zero() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    writeln!(tmp, "a: 1").unwrap();
    let path = tmp.path().to_str().unwrap();

    let (stdout, _stderr, code) = run_binary(&["--plain", path]);
    assert_eq!(code, 0, "stderr: {_stderr}");
    assert!(stdout.contains("Budget report"), "stdout: {stdout}");
}

#[test]
fn invalid_yaml_file_exits_three() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    // Intentionally broken YAML: mapping value not allowed here
    writeln!(tmp, "a: b: c:").unwrap();
    let path = tmp.path().to_str().unwrap();

    let (_stdout, stderr, code) = run_binary(&[path]);
    assert_eq!(code, 3, "stderr: {stderr}");
    // With miette feature, error uses fancy formatting without the word "invalid"
    assert!(
        stderr.contains("invalid") || stderr.contains("not allowed"),
        "stderr: {stderr}"
    );
}

#[test]
fn invalid_yaml_file_plain_mode_exits_three() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    writeln!(tmp, "a: b: c:").unwrap();
    let path = tmp.path().to_str().unwrap();

    let (_stdout, stderr, code) = run_binary(&["--plain", path]);
    assert_eq!(code, 3, "stderr: {stderr}");
    // Plain mode always includes "invalid" in the output
    assert!(stderr.contains("invalid"), "stderr: {stderr}");
}
