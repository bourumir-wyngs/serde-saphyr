#![cfg(all(feature = "serialize", feature = "deserialize"))]
//! Integration tests for the `serde-saphyr` CLI entrypoint.
#![cfg(all(not(miri), not(target_os = "wasi")))]
#![cfg(all(feature = "include", feature = "include_fs"))]

use std::io::Write;

/// Helper: run the CLI entrypoint in-process and return (stdout, stderr, exit_code).
fn run_binary(args: &[&str]) -> (String, String, i32) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = serde_saphyr::cli::run(args.iter().copied(), &mut stdout, &mut stderr);
    let stdout = String::from_utf8(stdout).expect("stdout is not valid UTF-8");
    let stderr = String::from_utf8(stderr).expect("stderr is not valid UTF-8");
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

#[test]
fn include_flag_parses_successfully() {
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let root_path = tmp_dir.path().join("root.yaml");
    let included_path = tmp_dir.path().join("included.yaml");

    std::fs::write(&root_path, "a: !include included.yaml\n").unwrap();
    std::fs::write(&included_path, "b: 2\n").unwrap();

    let root_path_str = root_path.to_str().unwrap();
    let dir_str = tmp_dir.path().to_str().unwrap();

    let (stdout, stderr, code) = run_binary(&["--include", dir_str, root_path_str]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("Budget report"), "stdout: {stdout}");
}

#[test]
fn missing_include_path_prints_error_and_exits_one() {
    let (_stdout, stderr, code) = run_binary(&["--include"]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("Missing path for --include"),
        "stderr: {stderr}"
    );
}
