use std::collections::HashMap;

// LQZ7: Double Quoted Implicit Keys
// "implicit block key": [ { "implicit flow key": "value" } ]
#[derive(Debug, serde::Deserialize)]
struct Root {
    #[serde(rename = "implicit block key")]
    block: Vec<HashMap<String, String>>,
}

#[test]
fn yaml_lqz7_double_quoted_implicit_keys() {
    let y = r#""implicit block key" : [
  "implicit flow key" : value,
 ]
"#;
    let v: Root = serde_saphyr::from_str(y).expect("failed to parse LQZ7");
    assert_eq!(v.block.len(), 1);
    let m = &v.block[0];
    assert_eq!(
        m.get("implicit flow key").map(String::as_str),
        Some("value")
    );
}
