use std::collections::HashMap;

// 4MUZ: Flow mapping colon on line after key
// The YAML file shows three variants that should all parse to {"foo": "bar"}.
#[test]
fn yaml_4muz_flow_mapping_colon_on_next_line() {
    let y1 = "{\"foo\"\n: \"bar\"}\n";
    let m1: HashMap<String, String> = serde_saphyr::from_str(y1).expect("4MUZ v1 parse");
    assert_eq!(m1.get("foo").map(String::as_str), Some("bar"));

    let y2 = "{\"foo\"\n: bar}\n";
    let m2: HashMap<String, String> = serde_saphyr::from_str(y2).expect("4MUZ v2 parse");
    assert_eq!(m2.get("foo").map(String::as_str), Some("bar"));

    let y3 = "{foo\n: bar}\n";
    let m3: HashMap<String, String> = serde_saphyr::from_str(y3).expect("4MUZ v3 parse");
    assert_eq!(m3.get("foo").map(String::as_str), Some("bar"));
}
