use serde_json::Value;

/// # YAML 1.2 Example 8.2 – Block Indentation Indicator (Test R4YG)
#[test]
fn y_r4yg() {
    let yaml = r#"
---
- |
  detected
- |


  # detected
- |2
    explicit
- |

  detected
    "#;

    // Expected scalar contents (exactly as YAML 1.2 specifies)
    let expected: [&str; 4] = [
        "detected\n",
        "\n\n# detected\n",
        "  explicit\n",
        "\ndetected\n  \n",
    ];

    let parsed: Vec<Value> = serde_saphyr::from_str(yaml).unwrap();

    // Compare parsed YAML scalars to expected
    assert_eq!(parsed.len(), expected.len());

    for (i, val) in parsed.iter().enumerate() {
        let parsed_str = val.as_str().unwrap();
        assert_eq!(parsed_str, expected[i], "Mismatch at index {}", i);
    }
}
