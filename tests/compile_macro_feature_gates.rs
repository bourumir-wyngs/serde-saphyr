// `cargo check` is spawned to verify feature-specific API and macro diagnostics.
// WebAssembly targets cannot launch it, and Miri cannot run process-spawning tests.
#![cfg(not(target_family = "wasm"))]
#![cfg(not(miri))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn write_fixture(
    root: &Path,
    package_name: &str,
    enabled_features: &[&str],
    source: &str,
) -> PathBuf {
    let dir = root.join(package_name);
    fs::create_dir_all(dir.join("src")).expect("create fixture source directory");

    let manifest_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    let enabled_features = enabled_features
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{package_name}"
version = "0.0.0"
edition = "2021"

[dependencies]
serde-saphyr = {{ path = "{manifest_dir}", default-features = false, features = [{enabled_features}] }}
"#
        ),
    )
    .expect("write fixture manifest");
    fs::write(dir.join("src/main.rs"), source).expect("write fixture source");

    // CI prefetches the Git revision pinned by the project lockfile. A fresh
    // lockfile would resolve branch heads again, but an offline Git cache may
    // contain only the pinned commit, without the branch's remote-tracking ref.
    let lockfile = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    fs::copy(lockfile, dir.join("Cargo.lock")).expect("copy repository lockfile into fixture");
    dir
}

fn cargo_check(dir: &Path, target_dir: &Path) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .current_dir(dir)
        .arg("check")
        .arg("--offline")
        .arg("--quiet")
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .expect("run cargo check")
}

fn assert_missing_feature_diagnostics(
    output: &Output,
    package_name: &str,
    macro_names: &[&str],
    required_feature: &str,
) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "{package_name} unexpectedly compiled\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    for macro_name in macro_names {
        let expected =
            format!("serde-saphyr `{macro_name}!` requires feature `{required_feature}`");
        assert!(
            stderr.contains(&expected),
            "missing friendly feature error `{expected}` for {package_name}:\n{stderr}"
        );
    }
}

#[test]
fn public_macros_report_missing_features() {
    let root = tempfile::tempdir().expect("create fixture root");
    let target_dir = root.path().join("target");

    let serialize_only = write_fixture(
        root.path(),
        "serde-saphyr-serialize-only-macro-gates",
        &["serialize"],
        r#"fn main() {
    let _ = serde_saphyr::options! {};
    let _ = serde_saphyr::budget! {};
    let _ = serde_saphyr::alias_limits! {};
    let _ = serde_saphyr::render_options! {};
}
"#,
    );
    let output = cargo_check(&serialize_only, &target_dir);
    assert_missing_feature_diagnostics(
        &output,
        "serialize-only macro fixture",
        &["options", "budget", "alias_limits", "render_options"],
        "deserialize",
    );

    let deserialize_only = write_fixture(
        root.path(),
        "serde-saphyr-deserialize-only-macro-gates",
        &["deserialize"],
        "fn main() { let _ = serde_saphyr::ser_options! {}; }\n",
    );
    let output = cargo_check(&deserialize_only, &target_dir);
    assert_missing_feature_diagnostics(
        &output,
        "deserialize-only macro fixture",
        &["ser_options"],
        "serialize",
    );
}

#[test]
fn comment_apis_require_parser_comments() {
    let root = tempfile::tempdir().expect("create fixture root");
    let target_dir = root.path().join("target");
    let source = r#"use serde_saphyr::{CommentPosition, Commented};

fn main() {
    let _ = Commented(42, String::from("answer"));
    let _ = serde_saphyr::ser_options! { comment_position: CommentPosition::Above };
    let _ = serde_saphyr::options! { emit_comments: false };
    let _ = serde_saphyr::budget! {
        max_total_comment_bytes: 0,
        max_buffered_comment_events: 0,
    };
    let _ = serde_saphyr::budget::BudgetReport::default().total_comment_bytes;
    let _ = serde_saphyr::budget::BudgetBreach::CommentBytes { total_comment_bytes: 1 };
}
"#;

    let without_comments = write_fixture(
        root.path(),
        "serde-saphyr-without-comment-apis",
        &["serialize", "deserialize"],
        source,
    );
    let output = cargo_check(&without_comments, &target_dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "comment APIs unexpectedly compiled without parser-comments"
    );
    for missing_api in [
        "Commented",
        "CommentPosition",
        "comment_position",
        "emit_comments",
        "max_total_comment_bytes",
        "max_buffered_comment_events",
        "total_comment_bytes",
        "CommentBytes",
    ] {
        assert!(
            stderr.contains(missing_api),
            "missing compile error for {missing_api}:\n{stderr}"
        );
    }

    let with_comments = write_fixture(
        root.path(),
        "serde-saphyr-with-comment-apis",
        &["serialize", "deserialize", "parser-comments"],
        source,
    );
    let output = cargo_check(&with_comments, &target_dir);
    assert!(
        output.status.success(),
        "comment APIs should compile with parser-comments:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
