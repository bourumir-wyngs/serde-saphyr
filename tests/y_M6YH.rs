use std::collections::HashMap;

// M6YH: Block sequence indentation — sequence with three elements:
// 1) literal block scalar "x\n"
// 2) mapping { foo: bar }
// 3) sequence [42]
#[test]
fn yaml_m6yh_block_sequence_indentation() {
    let y = r#"- |
 x
-
 foo: bar
-
 - 42
"#;

    let v: (String, HashMap<String, String>, Vec<i32>) =
        serde_saphyr::from_str(y).expect("failed to parse M6YH");
    assert_eq!(v.0.as_str(), "x\n");
    assert_eq!(v.1.get("foo").map(String::as_str), Some("bar"));
    assert_eq!(v.2, vec![42]);
}
