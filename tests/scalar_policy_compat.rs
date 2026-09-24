//! Standard scalar schemas and Serde's configurable policy retain their distinct rules.
// These tests deliberately exercise the deprecated flags for backward compatibility.
#![allow(deprecated)]

use serde_saphyr::scalar::{ScalarError, ScalarKind, ScalarStyle, Schema, resolve};

fn kind(text: &str, schema: Schema) -> ScalarKind {
    resolve(text, ScalarStyle::Plain, None, schema)
        .unwrap()
        .kind()
}

#[test]
fn specific_schema_exposes_independent_serde_boolean_and_octal_policies() {
    for strict_booleans in [false, true] {
        for legacy_octal_numbers in [false, true] {
            let schema = serde_saphyr::specific! { strict_booleans: strict_booleans, legacy_octal_numbers: legacy_octal_numbers };
            let scalar = |text| resolve(text, ScalarStyle::Plain, None, schema).unwrap();
            assert_eq!(scalar("TrUe").to_bool(), Ok(true));
            assert_eq!(scalar("nUlL").kind(), ScalarKind::Null);
            if strict_booleans {
                assert_eq!(scalar("yEs").kind(), ScalarKind::String);
            } else {
                assert_eq!(scalar("yEs").to_bool(), Ok(true));
            }
            if legacy_octal_numbers {
                assert_eq!(scalar("-010").to_i128(), Ok(-8));
                assert_eq!(scalar("0x_10").to_u128(), Ok(16));
            } else {
                assert_eq!(scalar("-010").to_f64(), Ok(-10.0));
                assert_eq!(scalar("0x_10").kind(), ScalarKind::String);
            }
        }
    }
}

#[test]
fn specific_schema_preserves_text_and_numeric_conversion_limits() {
    let schema = serde_saphyr::specific! { strict_booleans: true };
    for (text, expected) in [("+0Xf_f", 255), ("-0B10", -2), ("1_000", 1000)] {
        let scalar = resolve(text, ScalarStyle::Plain, None, schema).unwrap();
        assert_eq!(scalar.text(), text);
        assert_eq!(scalar.to_i128(), Ok(expected));
    }
    let beyond_u128 = "340282366920938463463374607431768211456";
    let scalar = resolve(beyond_u128, ScalarStyle::Plain, None, schema).unwrap();
    assert_eq!(scalar.kind(), ScalarKind::Integer);
    assert_eq!(scalar.to_u128(), Err(ScalarError::OutOfRange));
    let scalar = resolve("1e999", ScalarStyle::Plain, None, schema).unwrap();
    assert_eq!(scalar.kind(), ScalarKind::Float);
    assert_eq!(scalar.to_f64(), Err(ScalarError::OutOfRange));
    assert!(
        resolve("+.nAn", ScalarStyle::Plain, None, schema)
            .unwrap()
            .to_f64()
            .unwrap()
            .is_nan()
    );
    for text in [
        "1__0",
        "0x",
        "0b2",
        " 42",
        "42 ",
        "inf",
        "nan",
        "1:20",
        "2026-09-09",
    ] {
        assert_eq!(kind(text, schema), ScalarKind::String, "{text}");
    }
}

#[test]
fn specific_schema_respects_styles_and_validates_explicit_tags() {
    let schema = serde_saphyr::specific! { strict_booleans: true };
    let text = "0B10";
    assert_eq!(
        resolve(text, ScalarStyle::DoubleQuoted, None, schema)
            .unwrap()
            .kind(),
        ScalarKind::String
    );
    assert_eq!(
        resolve(
            text,
            ScalarStyle::DoubleQuoted,
            Some("tag:yaml.org,2002:int"),
            schema
        )
        .unwrap()
        .to_u128(),
        Ok(2)
    );
    assert_eq!(
        resolve(
            "yes",
            ScalarStyle::Plain,
            Some("tag:yaml.org,2002:bool"),
            schema
        )
        .unwrap_err(),
        ScalarError::InvalidValue {
            kind: ScalarKind::Boolean
        }
    );
}

#[test]
fn specific_quote_all_has_no_effect_on_resolution_or_conversions() {
    for strict_booleans in [false, true] {
        for legacy_octal_numbers in [false, true] {
            for text in [
                "word", "", "TrUe", "yes", "010", "0x_10", "null", "1.5", "1e999",
            ] {
                for style in [ScalarStyle::Plain, ScalarStyle::DoubleQuoted] {
                    for tag in [None, Some("tag:yaml.org,2002:int")] {
                        let result = |quote_all| {
                            resolve(
                                text,
                                style,
                                tag,
                                serde_saphyr::specific! { strict_booleans: strict_booleans, legacy_octal_numbers: legacy_octal_numbers, quote_all: quote_all },
                            )
                            .map(|scalar| {
                                (
                                    scalar.kind(),
                                    scalar.text(),
                                    scalar.to_bool(),
                                    scalar.to_i128(),
                                    scalar.to_u128(),
                                    scalar.to_f64(),
                                )
                            })
                        };
                        assert_eq!(result(false), result(true), "{text:?}, {style:?}, {tag:?}");
                    }
                }
            }
        }
    }
}

#[cfg(feature = "deserialize")]
mod deserialize {
    use super::*;
    use serde_json::Value;
    use serde_saphyr::{from_str, from_str_with_options, options};

    #[test]
    fn mixed_case_serde_booleans_and_null_remain_more_permissive_than_schemas() {
        for schema in [Schema::Yaml11, Schema::Yaml12] {
            for text in ["TrUe", "FaLsE", "yEs", "oFf", "nUlL"] {
                assert_eq!(kind(text, schema), ScalarKind::String, "{text}");
            }
        }
        for strict_booleans in [false, true] {
            let opts = options! { strict_booleans: strict_booleans };
            for (text, expected) in [("TrUe", true), ("FaLsE", false)] {
                assert_eq!(
                    from_str_with_options::<bool>(text, opts.clone()).unwrap(),
                    expected
                );
                assert_eq!(
                    from_str_with_options::<Value>(text, opts.clone()).unwrap(),
                    Value::Bool(expected)
                );
            }
            for (text, expected) in [("yEs", true), ("oFf", false)] {
                let typed = from_str_with_options::<bool>(text, opts.clone());
                let inferred = from_str_with_options::<Value>(text, opts.clone()).unwrap();
                if strict_booleans {
                    assert!(typed.is_err());
                    assert_eq!(inferred, Value::String(text.into()));
                } else {
                    assert_eq!(typed.unwrap(), expected);
                    assert_eq!(inferred, Value::Bool(expected));
                }
            }
            assert_eq!(
                from_str_with_options::<Value>("nUlL", opts).unwrap(),
                Value::Null
            );
        }
    }

    #[test]
    fn strict_booleans_does_not_disable_serde_numeric_extensions() {
        let opts = options! { strict_booleans: true };
        for (text, expected) in [
            ("0b10", 2),
            ("-0x10", -16),
            ("+0o10", 8),
            ("0X10", 16),
            ("0B10", 2),
            ("1_000", 1000),
        ] {
            assert_eq!(kind(text, Schema::Yaml12), ScalarKind::String, "{text}");
            assert_eq!(
                from_str_with_options::<i64>(text, opts.clone()).unwrap(),
                expected,
                "{text}"
            );
            assert_eq!(
                from_str_with_options::<Value>(text, opts.clone()).unwrap(),
                Value::from(expected),
                "{text}"
            );
        }
        // YAML 1.1 permits repeated separators; Serde deliberately does not.
        assert_eq!(kind("1__0", Schema::Yaml11), ScalarKind::Integer);
        assert!(from_str::<i64>("1__0").is_err());
        assert_eq!(from_str::<Value>("1__0").unwrap(), Value::from("1__0"));
    }

    #[test]
    fn legacy_octal_is_independent_of_the_boolean_policy() {
        assert_eq!(kind("010", Schema::Yaml12), ScalarKind::Integer);
        for strict_booleans in [false, true] {
            for legacy_octal_numbers in [false, true] {
                let opts = options! { strict_booleans: strict_booleans, legacy_octal_numbers: legacy_octal_numbers };
                for (text, decimal, octal) in [("010", 10.0, 8), ("-010", -10.0, -8)] {
                    let typed = from_str_with_options::<i64>(text, opts.clone());
                    let inferred = from_str_with_options::<Value>(text, opts.clone()).unwrap();
                    if legacy_octal_numbers {
                        assert_eq!(typed.unwrap(), octal);
                        assert_eq!(inferred, Value::from(octal));
                    } else {
                        assert!(typed.is_err());
                        assert!(inferred.is_f64());
                        assert_eq!(inferred.as_f64(), Some(decimal));
                    }
                }
                let prefixed = from_str_with_options::<u64>("0x_10", opts);
                if legacy_octal_numbers {
                    assert_eq!(prefixed.unwrap(), 16);
                } else {
                    assert!(prefixed.is_err());
                }
            }
        }
    }

    #[test]
    fn typeless_integer_overflow_retains_float_or_string_fallback() {
        assert_eq!(
            from_str::<Value>("18446744073709551615").unwrap(),
            Value::from(u64::MAX)
        );
        for text in ["18446744073709551616", "0x10000000000000000"] {
            let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
            assert_eq!(scalar.kind(), ScalarKind::Integer);
            assert_eq!(scalar.to_u128().unwrap(), u128::from(u64::MAX) + 1);
            assert_eq!(from_str::<u128>(text).unwrap(), u128::from(u64::MAX) + 1);
        }
        let inferred = from_str::<Value>("18446744073709551616").unwrap();
        assert!(inferred.is_f64());
        assert_eq!(inferred.as_f64(), Some(18446744073709551616.0));
        assert_eq!(
            from_str::<Value>("0x10000000000000000").unwrap(),
            Value::from("0x10000000000000000")
        );
    }

    #[test]
    fn serde_nonfinite_policy_preserves_extended_nan_and_overflow_handling() {
        assert_eq!(kind("+.nAn", Schema::Yaml12), ScalarKind::String);
        assert!(from_str::<f64>("+.nAn").unwrap().is_nan());
        assert!(from_str::<f64>("1e999").is_err());
        for (text, canonical) in [("+.nAn", ".nan"), ("1e999", ".inf")] {
            let error = from_str::<Value>(text).unwrap_err();
            assert!(matches!(
                error.without_snippet(),
                serde_saphyr::Error::NonFiniteFloat { .. }
            ));
            assert_eq!(
                from_str_with_options::<Value>(
                    text,
                    options! { reject_non_finite_typeless_float: false },
                )
                .unwrap(),
                Value::from(canonical)
            );
        }
    }
}

#[cfg(feature = "serialize")]
mod serialize {
    use super::*;
    use serde_saphyr::{FlowMap, ser_options, to_string_with_options};
    use std::collections::BTreeMap;

    fn assert_quoted_in_keys_and_values(text: &str, yaml_12: bool) {
        let opts = ser_options! { yaml_12: yaml_12, no_lang_directive: true };
        let map = BTreeMap::from([(text, text)]);
        let scalar = to_string_with_options(&text, opts.clone()).unwrap();
        let block = to_string_with_options(&map, opts.clone()).unwrap();
        let flow = to_string_with_options(&FlowMap(&map), opts).unwrap();
        for (yaml, count) in [(&scalar, 1), (&block, 2), (&flow, 2)] {
            let quoted = [format!("'{text}'"), serde_json::to_string(text).unwrap()]
                .iter()
                .map(|spelling| yaml.matches(spelling).count())
                .sum::<usize>();
            assert_eq!(quoted, count, "{text:?}, yaml_12={yaml_12}: {yaml}");
        }
        #[cfg(feature = "deserialize")]
        {
            assert_eq!(serde_saphyr::from_str::<String>(&scalar).unwrap(), text);
            for yaml in [&block, &flow] {
                assert_eq!(
                    serde_saphyr::from_str::<BTreeMap<String, String>>(yaml).unwrap(),
                    BTreeMap::from([(text.to_owned(), text.to_owned())])
                );
            }
        }
    }

    #[test]
    fn yaml12_quoting_still_protects_against_reader_extensions() {
        for text in [
            "0b10", "+0x10", "0X10", "1_0", "-.nAn", "NuLl", "TrUe", "NaN", "inf",
        ] {
            assert_eq!(kind(text, Schema::Yaml12), ScalarKind::String, "{text}");
            for yaml_12 in [false, true] {
                assert_quoted_in_keys_and_values(text, yaml_12);
            }
        }
    }

    #[test]
    fn yaml11_quoting_retains_go_and_unconstructible_numeric_extensions() {
        for text in [
            "0b+1",
            "0o_+7",
            "0b_",
            "1.2.3",
            "2026-9-9",
            "2026-09-09T12:34:56,123Z",
        ] {
            assert_eq!(kind(text, Schema::Yaml11), ScalarKind::String, "{text}");
            assert_quoted_in_keys_and_values(text, false);
            assert_eq!(
                to_string_with_options(
                    &text,
                    ser_options! { yaml_12: true, no_lang_directive: true },
                )
                .unwrap(),
                format!("{text}\n")
            );
        }
    }
}
