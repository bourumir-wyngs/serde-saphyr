#[test]
fn serialize_small_float_scientific_notation() {
    let value = 0.000004_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e-6");

    let value = 4.5123456e-18_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.5123456e-18");
}

#[test]
fn serialize_large_float_scientific_notation() {
    let value = 40000000000000000000.0_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e+19");
}

#[test]
fn serialize_positive_exponent_requires_plus_sign() {
    // YAML 1.1 requires an explicit sign in the exponent.
    let value = 3.12e18_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "3.12e+18");
}

#[test]
fn roundtrip_floats() {
    for original in [4.0e-6, 3.12e18, 17.4] {
        let yaml = serde_saphyr::to_string(&original).unwrap();
        let parsed: f64 = serde_saphyr::from_str(&yaml).unwrap();
        assert_eq!(original, parsed);
    }
}

use regex::Regex;

/// Validate that a plain scalar is recognized as `!!float` by:
/// - YAML 1.1 float tag regexp (base-10)
/// - YAML 1.2.2 JSON schema float tag resolution regexp
/// - YAML 1.2.2 Core schema float tag resolution regexp
///
/// Spec sources:
/// - YAML 1.1 float tag repository: https://yaml.org/type/float.html
/// - YAML 1.2.2 JSON/Core schema tag resolution: https://yaml.org/spec/1.2.2/
fn assert_yaml11_and_yaml12_float_scalar(s: &str) {
    // YAML 1.1 `!!float` (base 10) regexp (as published in the YAML 1.1 type repo).
    // NOTE: The published regexp has a known issue allowing extra '.' characters,
    // but it's still sufficient for validating our canonical-ish outputs.
    let yaml11_base10 =
        Regex::new(r"^[-+]?([0-9][0-9_]*)?\.[0-9.]*([eE][-+][0-9]+)?$").expect("valid regex");

    // YAML 1.2.2 JSON schema float tag resolution regexp:
    // `-? ( 0 | [1-9] [0-9]* ) ( \. [0-9]* )? ( [eE] [-+]? [0-9]+ )?`
    let yaml12_json =
        Regex::new(r"^-?(?:0|[1-9][0-9]*)(?:\.[0-9]*)?(?:[eE][-+]?[0-9]+)?$").expect("valid regex");

    // YAML 1.2.2 Core schema float tag resolution regexp (number form):
    // `[-+]? ( \. [0-9]+ | [0-9]+ ( \. [0-9]* )? ) ( [eE] [-+]? [0-9]+ )?`
    let yaml12_core = Regex::new(r"^[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)(?:[eE][-+]?[0-9]+)?$")
        .expect("valid regex");

    assert!(
        yaml11_base10.is_match(s),
        "not YAML 1.1 base-10 float literal: {s:?}"
    );
    assert!(
        yaml12_json.is_match(s),
        "not YAML 1.2 JSON-schema float literal: {s:?}"
    );
    assert!(
        yaml12_core.is_match(s),
        "not YAML 1.2 Core-schema float literal: {s:?}"
    );
}

#[test]
fn serialize_small_float_scientific_notation_2() {
    // --------------------
    // f64 cases
    // --------------------
    let value = 0.000004_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e-6");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    let value = -0.000004_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "-4.0e-6");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    let value = 4.5123456e-18_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.5123456e-18");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    let value = -4.5123456e-18_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "-4.5123456e-18");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    // --------------------
    // f32 cases
    // --------------------
    let value = 0.000000004_f32;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e-9");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    let value = -0.000000004_f32;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "-4.0e-9");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());
}

#[test]
fn serialize_large_float_scientific_notation_2() {
    // --------------------
    // f64 cases
    // --------------------
    let value = 40000000000000000000.0_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e+19");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    let value = -40000000000000000000.0_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "-4.0e+19");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    // --------------------
    // f32 cases
    // --------------------
    let value = 40000000000000000000.0_f32;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e+19");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());

    let value = -40000000000000000000.0_f32;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "-4.0e+19");
    assert_yaml11_and_yaml12_float_scalar(yaml.trim());
}
