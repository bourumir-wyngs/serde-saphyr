use std::collections::HashMap;

// 6H3V: Backslashes in singlequotes
#[test]
fn yaml_6h3v_backslashes_in_singlequotes() {
    // The key contains a backslash before the closing single quote; the value ends with a single quote.
    let y = "'foo: bar\\': baz'\n";
    let m: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse 6H3V");
    assert_eq!(m.get("foo: bar\\").map(String::as_str), Some("baz'"));
}
