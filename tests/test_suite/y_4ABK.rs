use std::collections::HashMap;

// 4ABK: Flow Mapping Separate Values
// Expect mapping with keys: "unquoted" -> "separate"; "http://foo.com" -> null; "omitted value" -> null
#[test]
fn yaml_4abk_flow_mapping_separate_values() {
    let y = "{\nunquoted : \"separate\",\nhttp://foo.com,\nomitted value:,\n}\n";
    let map: HashMap<String, Option<String>> =
        serde_saphyr::from_str(y).expect("failed to parse 4ABK");
    assert_eq!(
        map.get("unquoted").cloned().flatten().as_deref(),
        Some("separate")
    );
    assert_eq!(map.get("http://foo.com").and_then(|v| v.clone()), None);
    assert_eq!(map.get("omitted value").and_then(|v| v.clone()), None);
    assert_eq!(map.len(), 3);
}
