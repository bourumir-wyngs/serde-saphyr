use std::collections::HashMap;

// 2EBW: Allowed characters in keys
// The YAML snippet defines a mapping with keys that contain special characters
// like question mark, colon, dash, and hash (#) that should be treated as part
// of the key in plain style, not as indicators when used in these positions.
// We deserialize into a HashMap<String, String> and assert all values.
#[test]
fn yaml_2ebw_allowed_chars_in_keys() {
    let yaml = concat!(
        "a!\"#$%&'()*+,-./09:;<=>?@AZ[\\]^_`az{|}~: safe\n",
        "?foo: safe question mark\n",
        ":foo: safe colon\n",
        "-foo: safe dash\n",
        "this is#not: a comment\n",
    );

    let map: HashMap<String, String> =
        serde_saphyr::from_str(yaml).expect("failed to parse 2EBW YAML");

    assert_eq!(
        map.get("a!\"#$%&'()*+,-./09:;<=>?@AZ[\\]^_`az{|}~"),
        Some(&"safe".to_string())
    );
    assert_eq!(map.get("?foo"), Some(&"safe question mark".to_string()));
    assert_eq!(map.get(":foo"), Some(&"safe colon".to_string()));
    assert_eq!(map.get("-foo"), Some(&"safe dash".to_string()));
    assert_eq!(map.get("this is#not"), Some(&"a comment".to_string()));
    assert_eq!(map.len(), 5);
}
