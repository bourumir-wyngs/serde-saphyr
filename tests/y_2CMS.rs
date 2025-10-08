use serde::Deserialize;

// 2CMS: "Invalid mapping in plain multiline" — the snippet under the `yaml:` key
// in tests/yaml-test-suite/src/2CMS.yaml is intentionally invalid YAML.
// The test asserts that parsing this input fails (and does not panic).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy {
    k: String,
}

#[test]
fn yaml_2cms_invalid_mapping_in_plain_multiline_fails() {
    let yaml = "this\n is\n  invalid: x\n";

    let result: Result<Dummy, _> = serde_saphyr::from_str(yaml);
    assert!(result.is_err(), "2CMS snippet should fail to parse, but it succeeded: {:?}", result);
}
