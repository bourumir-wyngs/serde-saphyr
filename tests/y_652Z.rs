use std::collections::HashMap;

// 652Z: Question mark at start of flow key — map with keys "?foo" and "bar"
#[test]
fn yaml_652z_question_mark_at_start_of_flow_key() {
    let y = "{ ?foo: bar,\nbar: 42\n}\n";
    // Accept values as strings for simplicity; the integer 42 will deserialize as "42" when targeting String.
    let m: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse 652Z");
    assert_eq!(m.get("?foo").map(String::as_str), Some("bar"));
    // Either "42" (as string) or, if desired, parse to an integer separately — here we compare as string.
    assert_eq!(m.get("bar").map(String::as_str), Some("42"));
}
