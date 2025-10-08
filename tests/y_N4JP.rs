use serde::Deserialize;

// N4JP: Bad indentation in mapping (fail: true) — expect error
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Root { map: std::collections::HashMap<String, String> }

#[test]
fn yaml_n4jp_bad_indentation_should_fail() {
    let y = r#"map:
  key1: "quoted1"
 key2: "bad indentation"
"#;
    let result: Result<Root, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "N4JP should fail to parse due to bad indentation");
}
