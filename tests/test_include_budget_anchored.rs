#![cfg(feature = "include_fs")]

use serde::Deserialize;
use serde_saphyr::{from_reader_with_options, Options, SafeFileResolver};
use tempfile::tempdir;
use std::fs;

#[derive(Debug, Deserialize, PartialEq)]
struct Root {
    included1: String,
    included2: String,
}

#[test]
fn test_anchored_includes_exceed_budget() {
    let yaml = r#"
i1: !include "f.yml#f"
i2: !include "f.yml#f"
"#;

    let temp = tempdir().unwrap();
    let file_path = temp.path().join("f.yml");
    let file_content = "f: |\n  ".to_string() + &"a".repeat(80);
    fs::write(&file_path, file_content).unwrap();

    let resolver = SafeFileResolver::new(temp.path()).unwrap().into_callback();
    
    // root YAML ~50 bytes
    // f.yml ~85 bytes
    // Total needed > 220
    // Limit: 150
    // First include should pass: 50 + 85 = 135 < 150
    // Second include should fail: 135 + 85 = 220 > 150
    let mut options = Options::default().with_include_resolver(resolver);
    options.budget = Some(serde_saphyr::budget::Budget {
        max_reader_input_bytes: Some(150),
        ..Default::default()
    });

    let result: Result<Root, _> = from_reader_with_options(yaml.as_bytes(), options);
    assert!(result.is_err(), "Expected parsing to fail due to budget exhaustion");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("exceed"), "Error should mention exceeding limit, got: {}", err_msg);
}
