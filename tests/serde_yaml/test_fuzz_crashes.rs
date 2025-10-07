use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};
use serde_json::Value;

fn collect_test_inputs(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut inputs = Vec::new();
    if !base.exists() {
        return Ok(inputs);
    }
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
           inputs.push(path);
        }
    }
    Ok(inputs)
}

