#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_saphyr::{
    DuplicateKeyPolicy, Options, from_reader, from_reader_with_options, from_str,
    from_str_with_options,
};

// YAML 1.2 treats these non-ASCII characters as scalar content, even though
// Rust's str::trim considers them whitespace.
const UNICODE_WHITESPACE: &[char] = &[
    '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}',
    '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}',
    '\u{202f}', '\u{205f}', '\u{3000}',
];
const REPORTED_WHITESPACE: &[char] = &['\u{a0}', '\u{2003}', '\u{3000}', '\u{85}'];

fn padded(token: &str, whitespace: char) -> [String; 3] {
    [
        format!("{whitespace}{token}"),
        format!("{token}{whitespace}"),
        format!("{whitespace}{token}{whitespace}"),
    ]
}

fn assert_string_from_both(scalar: &str) {
    let expected = Value::String(scalar.to_owned());
    assert_eq!(
        from_str::<Value>(scalar).unwrap(),
        expected,
        "from_str changed {scalar:?}"
    );
    assert_eq!(
        from_reader::<_, Value>(scalar.as_bytes()).unwrap(),
        expected,
        "from_reader changed {scalar:?}"
    );
}

#[test]
fn unicode_edges_preserve_implicit_scalar_strings() {
    for &whitespace in UNICODE_WHITESPACE {
        for token in [
            "42", "-42", "0x2a", "1.5", "true", "false", "yes", "null", "~",
        ] {
            for scalar in padded(token, whitespace) {
                assert_string_from_both(&scalar);
            }
        }
    }
}

#[test]
fn unicode_edges_preserve_nonfinite_and_overflow_strings() {
    for &whitespace in REPORTED_WHITESPACE {
        for token in [".inf", "-.inf", ".nan", "1e999"] {
            for scalar in padded(token, whitespace) {
                assert_string_from_both(&scalar);
            }
        }
    }
}

#[test]
fn strict_boolean_inference_preserves_unicode_edges() {
    let options = serde_saphyr::options! { strict_booleans: true };
    for &whitespace in REPORTED_WHITESPACE {
        for token in ["true", "false"] {
            for scalar in padded(token, whitespace) {
                let expected = Value::String(scalar.clone());
                assert_eq!(
                    from_str_with_options::<Value>(&scalar, options.clone()).unwrap(),
                    expected,
                    "strict from_str changed {scalar:?}"
                );
                assert_eq!(
                    from_reader_with_options::<_, Value>(scalar.as_bytes(), options.clone())
                        .unwrap(),
                    expected,
                    "strict from_reader changed {scalar:?}"
                );
            }
        }
    }
}

fn assert_typed_rejected<T: DeserializeOwned>(yaml: &str, options: Options) {
    assert!(
        from_str_with_options::<T>(yaml, options.clone()).is_err(),
        "typed from_str accepted {yaml:?}"
    );
    assert!(
        from_reader_with_options::<_, T>(yaml.as_bytes(), options).is_err(),
        "typed from_reader accepted {yaml:?}"
    );
}

#[test]
fn typed_scalars_reject_unicode_edges_even_with_explicit_tags() {
    for &whitespace in REPORTED_WHITESPACE {
        for (token, tag) in [
            ("42", "int"),
            ("-42", "int"),
            ("1.5", "float"),
            ("true", "bool"),
        ] {
            for scalar in padded(token, whitespace) {
                let tagged = format!("!!{tag} '{scalar}'");
                assert_typed_rejected::<Value>(&tagged, Options::default());
                for yaml in [&scalar, &tagged] {
                    match tag {
                        "int" => {
                            assert_typed_rejected::<i64>(yaml, Options::default());
                            assert_typed_rejected::<u64>(yaml, Options::default());
                        }
                        "float" => assert_typed_rejected::<f64>(yaml, Options::default()),
                        "bool" => {
                            assert_typed_rejected::<bool>(yaml, Options::default());
                            assert_typed_rejected::<bool>(
                                yaml,
                                serde_saphyr::options! { strict_booleans: true },
                            );
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

#[test]
fn no_schema_accepts_owned_and_borrowed_unicode_padded_strings() {
    for strict_booleans in [false, true] {
        let options = serde_saphyr::options! { no_schema: true, strict_booleans: strict_booleans };
        for &whitespace in REPORTED_WHITESPACE {
            for token in ["42", "-42", "1.5", "true", "false", "yes", "null"] {
                for scalar in padded(token, whitespace) {
                    assert_eq!(
                        from_str_with_options::<String>(&scalar, options.clone()).unwrap(),
                        scalar
                    );
                    assert_eq!(
                        from_str_with_options::<&str>(&scalar, options.clone()).unwrap(),
                        scalar
                    );
                    assert_eq!(
                        from_reader_with_options::<_, String>(scalar.as_bytes(), options.clone())
                            .unwrap(),
                        scalar
                    );
                }
            }
        }
    }
}

#[test]
fn unicode_padded_numeric_keys_remain_distinct_under_every_duplicate_policy() {
    for &whitespace in REPORTED_WHITESPACE {
        let mut yaml = "42: unpadded\n".to_owned();
        let mut expected = BTreeMap::from([("42".to_owned(), "unpadded".to_owned())]);
        for (index, key) in padded("42", whitespace).into_iter().enumerate() {
            yaml.push_str(&format!("{key}: padded{index}\n"));
            expected.insert(key, format!("padded{index}"));
        }
        for policy in [
            DuplicateKeyPolicy::Error,
            DuplicateKeyPolicy::FirstWins,
            DuplicateKeyPolicy::LastWins,
        ] {
            let options = serde_saphyr::options! { duplicate_keys: policy };
            assert_eq!(
                from_str_with_options::<BTreeMap<String, String>>(&yaml, options.clone()).unwrap(),
                expected,
                "from_str merged keys under {policy:?}"
            );
            assert_eq!(
                from_reader_with_options::<_, BTreeMap<String, String>>(yaml.as_bytes(), options)
                    .unwrap(),
                expected,
                "from_reader merged keys under {policy:?}"
            );
        }
    }
}

#[test]
fn explicit_block_scalars_still_accept_crlf_whitespace() {
    for (yaml, expected) in [
        ("!!int |\r\n  -42\r\n", Value::from(-42)),
        ("!!float >\r\n  1.5\r\n", Value::from(1.5)),
        ("!!bool |\r\n  true\r\n", Value::Bool(true)),
    ] {
        assert_eq!(from_str::<Value>(yaml).unwrap(), expected);
        assert_eq!(from_reader::<_, Value>(yaml.as_bytes()).unwrap(), expected);
    }
}

#[test]
fn escaped_non_yaml_ascii_whitespace_is_rejected_in_typed_scalars() {
    for escape in ["\\v", "\\f"] {
        for (token, tag) in [("-42", "int"), ("1.5", "float"), ("true", "bool")] {
            for payload in [format!("{escape}{token}"), format!("{token}{escape}")] {
                let yaml = format!("!!{tag} \"{payload}\"");
                assert_typed_rejected::<Value>(&yaml, Options::default());
                match tag {
                    "int" => assert_typed_rejected::<i64>(&yaml, Options::default()),
                    "float" => assert_typed_rejected::<f64>(&yaml, Options::default()),
                    "bool" => assert_typed_rejected::<bool>(&yaml, Options::default()),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[cfg(feature = "serialize")]
#[test]
fn reported_numeric_string_mapping_roundtrips_without_quoting() {
    let original = BTreeMap::from([("k".to_owned(), "-11.5\u{a0}".to_owned())]);
    let yaml = serde_saphyr::to_string(&original).unwrap();
    assert_eq!(yaml, "k: -11.5\u{a0}\n");
    assert_eq!(
        from_str::<BTreeMap<String, String>>(&yaml).unwrap(),
        original
    );
    assert_eq!(
        from_reader::<_, BTreeMap<String, String>>(yaml.as_bytes()).unwrap(),
        original
    );
    let expected = serde_json::json!({ "k": "-11.5\u{a0}" });
    assert_eq!(from_str::<Value>(&yaml).unwrap(), expected);
    assert_eq!(from_reader::<_, Value>(yaml.as_bytes()).unwrap(), expected);
}

#[cfg(feature = "serialize")]
#[test]
fn unicode_padded_strings_roundtrip_through_untyped_values() {
    for &whitespace in REPORTED_WHITESPACE {
        for token in ["42", "1.5", "true", "yes", "null"] {
            for scalar in padded(token, whitespace) {
                let expected = Value::String(scalar);
                let yaml = serde_saphyr::to_string(&expected).unwrap();
                assert_eq!(
                    from_str::<Value>(&yaml).unwrap(),
                    expected,
                    "YAML: {yaml:?}"
                );
                assert_eq!(from_reader::<_, Value>(yaml.as_bytes()).unwrap(), expected);
            }
        }
    }
}
