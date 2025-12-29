use std::collections::HashMap;

// C2DT: Flow Mapping Adjacent Values
// Expect mapping with keys: "adjacent" -> "value", "readable" -> "value", "empty" -> null
#[test]
fn yaml_c2dt_flow_mapping_adjacent_values() {
    let y = r#"{
"adjacent":value,
"readable": value,
"empty":
}
"#;
    let m: HashMap<String, Option<String>> =
        serde_saphyr::from_str(y).expect("failed to parse C2DT");
    assert_eq!(
        m.get("adjacent").cloned().flatten().as_deref(),
        Some("value")
    );
    assert_eq!(
        m.get("readable").cloned().flatten().as_deref(),
        Some("value")
    );
    assert_eq!(m.get("empty").and_then(|v| v.clone()), None);
    assert_eq!(m.len(), 3);
}
