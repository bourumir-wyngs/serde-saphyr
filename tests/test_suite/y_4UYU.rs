use serde::Deserialize;
use std::collections::HashMap;

// 4UYU: Colon in Double Quoted String as a mapping key
// Define a structure that contains a HashMap field and some extra context.
#[derive(Debug, Deserialize, PartialEq)]
struct Wrapper {
    title: String,
    data: HashMap<String, String>,
}

#[test]
fn yaml_4uyu_colon_in_double_quoted_string_key_in_map() {
    // More complex YAML around a mapping that has a quoted key containing a colon.
    // The map should hold a single entry with the colon inside the name.
    let y = "title: Example\ndata:\n  \"foo: bar\": baz\n";
    let doc: Wrapper = serde_saphyr::from_str(y).expect("failed to parse 4UYU");

    assert_eq!(doc.title, "Example");
    assert_eq!(doc.data.len(), 1);
    assert_eq!(doc.data.get("foo: bar").map(String::as_str), Some("baz"));
}
