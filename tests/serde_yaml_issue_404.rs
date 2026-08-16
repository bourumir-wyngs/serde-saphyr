#![cfg(feature = "deserialize")]

// Regression coverage for https://github.com/dtolnay/serde-yaml/issues/404.
use serde_json::json;

#[test]
fn leading_colon_plain_scalars_parse_in_flow_collections() {
    let yaml = r#"
- a:
    b:
      c:
        - f: [:this_WILL_fail]
        - f: [':this_will_NOT_fail']
      e:
        - { g: :this_WILL_fail }
        - { g: ':this_will_NOT_fail' }
"#;

    let actual: serde_json::Value =
        serde_saphyr::from_str(yaml).expect("leading-colon scalars should deserialize");

    assert_eq!(
        actual,
        json!([{
            "a": {
                "b": {
                    "c": [
                        {"f": [":this_WILL_fail"]},
                        {"f": [":this_will_NOT_fail"]}
                    ],
                    "e": [
                        {"g": ":this_WILL_fail"},
                        {"g": ":this_will_NOT_fail"}
                    ]
                }
            }
        }])
    );
}
