// `cargo check` is spawned to verify feature-specific macro diagnostics.
// This is not supported on WASI runtimes, and Miri cannot run process-spawning tests.
#![cfg(not(target_os = "wasi"))]
#![cfg(not(miri))]

use std::fs;
use std::process::Command;

#[test]
fn render_options_requires_deserialize_feature() {
    let dir = tempfile::tempdir().expect("create temp crate");
    let manifest_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");

    fs::create_dir(dir.path().join("src")).expect("create src dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "serde-saphyr-render-options-feature-gate"
version = "0.0.0"
edition = "2021"

[dependencies]
serde-saphyr = {{ path = "{manifest_dir}", default-features = false, features = ["serialize"] }}
"#
        ),
    )
    .expect("write Cargo.toml");
    fs::write(
        dir.path().join("src/main.rs"),
        "fn main() { let _ = serde_saphyr::render_options! {}; }\n",
    )
    .expect("write main.rs");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(dir.path())
        .arg("check")
        .arg("--quiet")
        .env("CARGO_TARGET_DIR", dir.path().join("target"))
        .output()
        .expect("run cargo check");

    assert!(
        !output.status.success(),
        "render_options! unexpectedly compiled without deserialize\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("serde-saphyr `render_options!` requires feature `deserialize`"),
        "missing friendly render_options! feature error:\n{stderr}"
    );
}
