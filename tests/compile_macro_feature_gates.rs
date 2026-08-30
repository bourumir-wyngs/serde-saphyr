// `cargo check` is spawned to verify feature-specific macro diagnostics.
// WebAssembly targets cannot launch it, and Miri cannot run process-spawning tests.
#![cfg(not(target_family = "wasm"))]
#![cfg(not(miri))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn write_fixture(root: &Path, package_name: &str, enabled_feature: &str, source: &str) -> PathBuf {
    let dir = root.join(package_name);
    fs::create_dir_all(dir.join("src")).expect("create fixture source directory");

    let manifest_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{package_name}"
version = "0.0.0"
edition = "2021"

[dependencies]
serde-saphyr = {{ path = "{manifest_dir}", default-features = false, features = ["{enabled_feature}"] }}
"#
        ),
    )
    .expect("write fixture manifest");
    fs::write(dir.join("src/main.rs"), source).expect("write fixture source");

    dir
}

fn cargo_check(dir: &Path, target_dir: &Path) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .current_dir(dir)
        .arg("check")
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
        "serialize",
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
        "deserialize",
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
