#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::{self, Visitor};
use serde_json::Value;
use serde_saphyr::NonFiniteFloatPolicy;
use std::collections::BTreeMap;
#[cfg(feature = "properties")]
use std::collections::HashMap;
use std::fmt;

const NON_FINITE_CASES: &[(&str, f64, &str)] = &[
    (".nan", f64::NAN, ".nan"),
    (".NaN", f64::NAN, ".nan"),
    (".NAN", f64::NAN, ".nan"),
    ("+.nan", f64::NAN, ".nan"),
    ("-.NaN", f64::NAN, ".nan"),
    (".inf", f64::INFINITY, ".inf"),
    (".Inf", f64::INFINITY, ".inf"),
    (".INF", f64::INFINITY, ".inf"),
    ("+.INF", f64::INFINITY, ".inf"),
    ("-.inf", f64::NEG_INFINITY, "-.inf"),
    ("-.InF", f64::NEG_INFINITY, "-.inf"),
    ("1e999", f64::INFINITY, ".inf"),
    ("9e400", f64::INFINITY, ".inf"),
    ("-1e999", f64::NEG_INFINITY, "-.inf"),
];

const POLICIES: [NonFiniteFloatPolicy; 3] = [
    NonFiniteFloatPolicy::Reject,
    NonFiniteFloatPolicy::PassThrough,
    NonFiniteFloatPolicy::AsString,
];

/// A float-capable typeless consumer, such as a YAML value or a shell's value enum.
#[derive(Debug)]
struct AnyFloat(f64);

impl<'de> Deserialize<'de> for AnyFloat {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FloatVisitor;

        impl Visitor<'_> for FloatVisitor {
            type Value = AnyFloat;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a floating point value")
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(AnyFloat(value))
            }
        }

        deserializer.deserialize_any(FloatVisitor)
    }
}

fn assert_float(actual: f64, expected: f64, yaml: &str) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "yaml: {yaml}, value: {actual}");
    } else {
        assert_eq!(actual, expected, "yaml: {yaml}");
    }
}

#[test]
fn default_rejects_non_finite_typeless_floats_even_for_float_capable_visitors() {
    for &(yaml, _, _) in NON_FINITE_CASES {
        for err in [
            serde_saphyr::from_str::<AnyFloat>(yaml).unwrap_err(),
            serde_saphyr::from_str::<Value>(yaml).unwrap_err(),
        ] {
            assert!(
                matches!(
                    err.without_snippet(),
                    serde_saphyr::Error::NonFiniteFloat { value, .. } if value == yaml
                ),
                "yaml: {yaml}, error: {err}"
            );
        }
    }
}

#[test]
fn pass_through_delivers_non_finite_typeless_floats_to_the_visitor() {
    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    for &(yaml, expected, _) in NON_FINITE_CASES {
        let value: AnyFloat = serde_saphyr::from_str_with_options(yaml, opts.clone()).unwrap();
        assert_float(value.0, expected, yaml);
    }
}

#[test]
fn pass_through_leaves_non_finite_handling_to_the_json_visitor() {
    // serde_json::Value accepts non-finite floats by converting them to null.
    // Reject remains the default to prevent this loss unless explicitly requested.
    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    for &(yaml, _, _) in NON_FINITE_CASES {
        let value: Value = serde_saphyr::from_str_with_options(yaml, opts.clone()).unwrap();
        assert_eq!(value, Value::Null, "yaml: {yaml}");
    }
}

#[test]
fn reject_policy_rejects_non_finite_spellings_and_overflow() {
    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::Reject,
    };
    for &(literal, _, _) in NON_FINITE_CASES {
        let yaml = format!("x: {literal}");
        let err = serde_saphyr::from_str_with_options::<Value>(&yaml, opts.clone())
            .expect_err("Reject should reject non-finite typeless floats");
        assert!(
            matches!(
                err.without_snippet(),
                serde_saphyr::Error::NonFiniteFloat { value, .. } if value == literal
            ),
            "yaml: {yaml}, error: {err}"
        );
        let msg = err.to_string();
        assert!(msg.contains("non-finite float"), "unexpected error: {msg}");
        assert!(msg.contains(literal), "offending value missing: {msg}");
    }
}

#[test]
fn as_string_canonicalizes_non_finite_spellings_and_overflow() {
    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::AsString,
    };
    for &(yaml, _, canonical) in NON_FINITE_CASES {
        let value: Value = serde_saphyr::from_str_with_options(yaml, opts.clone()).unwrap();
        assert_eq!(value, Value::String(canonical.to_owned()), "yaml: {yaml}");
    }
}

#[test]
fn explicit_float_tags_follow_typeless_non_finite_policy() {
    for (literal, expected, canonical) in [
        ("+.NaN", f64::NAN, ".nan"),
        ("+.INF", f64::INFINITY, ".inf"),
        ("-.inf", f64::NEG_INFINITY, "-.inf"),
        ("1e999", f64::INFINITY, ".inf"),
        ("-1e999", f64::NEG_INFINITY, "-.inf"),
    ] {
        for yaml in [
            format!("!!float {literal}"),
            format!("!!float '{literal}'"),
            format!("!!float \"{literal}\""),
            format!("!!float |-\n  {literal}\n"),
            format!("!!float >-\n  {literal}\n"),
        ] {
            let opts = serde_saphyr::options! {
                non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
            };
            let value: AnyFloat = serde_saphyr::from_str_with_options(&yaml, opts).unwrap();
            assert_float(value.0, expected, &yaml);

            let opts = serde_saphyr::options! {
                non_finite_float_policy: NonFiniteFloatPolicy::Reject,
            };
            let err = serde_saphyr::from_str_with_options::<Value>(&yaml, opts).unwrap_err();
            assert!(
                matches!(
                    err.without_snippet(),
                    serde_saphyr::Error::NonFiniteFloat { value, .. } if value == literal
                ),
                "yaml: {yaml}, error: {err}"
            );

            let opts = serde_saphyr::options! {
                non_finite_float_policy: NonFiniteFloatPolicy::AsString,
            };
            let value: Value = serde_saphyr::from_str_with_options(&yaml, opts).unwrap();
            assert_eq!(value, Value::String(canonical.to_owned()), "yaml: {yaml}");
        }
    }
}

#[test]
fn policies_leave_finite_numbers_and_strings_alone() {
    for policy in POLICIES {
        let opts = serde_saphyr::options! {
            non_finite_float_policy: policy,
        };
        let value: Value = serde_saphyr::from_str_with_options("2.5", opts.clone()).unwrap();
        assert_eq!(value, Value::from(2.5));

        // A hostname must not be mistaken for an overflowing numeral.
        let value: Value =
            serde_saphyr::from_str_with_options("inf.example.com", opts.clone()).unwrap();
        assert_eq!(value, Value::String("inf.example.com".to_owned()));

        for literal in [".nan", "+.INF", "-.inf", "1e999"] {
            for yaml in [
                format!("'{literal}'"),
                format!("\"{literal}\""),
                format!("|-\n  {literal}\n"),
                format!(">-\n  {literal}\n"),
                format!("!!str {literal}"),
            ] {
                let value: Value =
                    serde_saphyr::from_str_with_options(&yaml, opts.clone()).unwrap();
                assert_eq!(value, Value::String(literal.to_owned()), "yaml: {yaml}");
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FloatOrString {
    Float(f64),
    String(String),
}

#[test]
fn pass_through_preserves_non_finite_floats_in_untagged_enums() {
    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    for &(yaml, expected, _) in NON_FINITE_CASES {
        let value: FloatOrString = serde_saphyr::from_str_with_options(yaml, opts.clone()).unwrap();
        let FloatOrString::Float(value) = value else {
            panic!("expected a float for {yaml}, got {value:?}");
        };
        assert_float(value, expected, yaml);
    }

    let value: FloatOrString = serde_saphyr::from_str_with_options("'.nan'", opts).unwrap();
    assert!(matches!(value, FloatOrString::String(value) if value == ".nan"));
}

#[test]
fn pass_through_preserves_non_finite_floats_in_flattened_maps() {
    #[derive(Debug, Deserialize)]
    struct Flattened {
        #[serde(flatten)]
        values: BTreeMap<String, f64>,
    }

    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    let value: Flattened =
        serde_saphyr::from_str_with_options("nan: .nan\npositive: .inf\nnegative: -.inf\n", opts)
            .unwrap();
    assert!(value.values["nan"].is_nan());
    assert_eq!(value.values["positive"], f64::INFINITY);
    assert_eq!(value.values["negative"], f64::NEG_INFINITY);
}

#[test]
fn pass_through_preserves_non_finite_aliases_from_reader_and_slice() {
    let yaml = b"[&value .nan, *value, &negative -.inf, *negative]";
    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    let reader_values: Vec<AnyFloat> =
        serde_saphyr::from_reader_with_options(&yaml[..], opts.clone()).unwrap();
    let slice_values: Vec<AnyFloat> = serde_saphyr::from_slice_with_options(yaml, opts).unwrap();
    for values in [reader_values, slice_values] {
        assert_eq!(values.len(), 4);
        assert!(values[0].0.is_nan());
        assert!(values[1].0.is_nan());
        assert_eq!(values[2].0, f64::NEG_INFINITY);
        assert_eq!(values[3].0, f64::NEG_INFINITY);
    }
}

#[test]
fn policies_do_not_affect_concrete_float_targets() {
    // These policies govern deserialize_any. Concrete float targets continue to
    // accept YAML non-finite spellings and reject overflowing decimal literals.
    for policy in POLICIES {
        let opts = serde_saphyr::options! {
            non_finite_float_policy: policy,
        };
        for (yaml, expected) in [
            (".nan", f64::NAN),
            (".inf", f64::INFINITY),
            ("-.inf", f64::NEG_INFINITY),
        ] {
            let value: f64 = serde_saphyr::from_str_with_options(yaml, opts.clone()).unwrap();
            assert_float(value, expected, yaml);
            let value: f32 = serde_saphyr::from_str_with_options(yaml, opts.clone()).unwrap();
            assert_float(f64::from(value), expected, yaml);
        }

        let err = serde_saphyr::from_str_with_options::<f64>("1e999", opts.clone())
            .expect_err("overflow remains invalid for a concrete f64 target");
        assert!(err.to_string().contains("invalid floating point"), "{err}");
        for yaml in ["1e999", "1e40"] {
            let err = serde_saphyr::from_str_with_options::<f32>(yaml, opts.clone())
                .expect_err("overflow remains invalid for a concrete f32 target");
            assert!(err.to_string().contains("invalid floating point"), "{err}");
        }
    }
}

#[cfg(feature = "properties")]
#[test]
fn reject_policy_reports_raw_interpolated_scalar() {
    let mut props = HashMap::new();
    props.insert("TIMEOUT".to_string(), "1e999".to_string());

    let opts = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::Reject,
    }
    .with_properties(props);

    let err = serde_saphyr::from_str_with_options::<Value>("${TIMEOUT}\n", opts)
        .expect_err("resolved overflowing literal should error");

    let msg = err.to_string();
    assert!(msg.contains("${TIMEOUT}"), "raw scalar missing: {msg}");
    assert!(!msg.contains("1e999"), "resolved value leaked: {msg}");
}
