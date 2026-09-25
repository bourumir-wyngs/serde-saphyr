#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::{self, Visitor};
use serde_json::Value;
use std::collections::BTreeMap;
#[cfg(feature = "properties")]
use std::collections::HashMap;
use std::fmt;

const NON_FINITE_CASES: &[(&str, f64)] = &[
    (".nan", f64::NAN),
    (".NaN", f64::NAN),
    (".NAN", f64::NAN),
    ("+.nan", f64::NAN),
    ("-.NaN", f64::NAN),
    (".inf", f64::INFINITY),
    (".Inf", f64::INFINITY),
    (".INF", f64::INFINITY),
    ("+.INF", f64::INFINITY),
    ("-.inf", f64::NEG_INFINITY),
    ("-.InF", f64::NEG_INFINITY),
    ("1e999", f64::INFINITY),
    ("9e400", f64::INFINITY),
    ("-1e999", f64::NEG_INFINITY),
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
fn default_passes_non_finite_typeless_floats_to_the_visitor() {
    for &(yaml, expected) in NON_FINITE_CASES {
        let value: AnyFloat = serde_saphyr::from_str(yaml).unwrap();
        assert_float(value.0, expected, yaml);
    }
}

#[test]
fn default_leaves_non_finite_handling_to_the_json_visitor() {
    // serde_json::Value accepts non-finite floats by converting them to null.
    // Consumers that need an error must enable reject_non_finite_typeless_float.
    for &(yaml, _) in NON_FINITE_CASES {
        let value: Value = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(value, Value::Null, "yaml: {yaml}");
    }
}

#[test]
fn reject_option_rejects_non_finite_spellings_and_overflow() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    };
    for &(literal, _) in NON_FINITE_CASES {
        let yaml = format!("x: {literal}");
        let err = serde_saphyr::from_str_with_options::<Value>(&yaml, opts.clone())
            .expect_err("enabled option should reject non-finite typeless floats");
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
fn explicit_float_tags_respect_non_finite_rejection_option() {
    for (literal, expected) in [
        ("+.NaN", f64::NAN),
        ("+.INF", f64::INFINITY),
        ("-.inf", f64::NEG_INFINITY),
        ("1e999", f64::INFINITY),
        ("-1e999", f64::NEG_INFINITY),
    ] {
        for yaml in [
            format!("!!float {literal}"),
            format!("!!float '{literal}'"),
            format!("!!float \"{literal}\""),
            format!("!!float |-\n  {literal}\n"),
            format!("!!float >-\n  {literal}\n"),
        ] {
            let opts = serde_saphyr::options! {
                reject_non_finite_typeless_float: false,
            };
            let value: AnyFloat = serde_saphyr::from_str_with_options(&yaml, opts).unwrap();
            assert_float(value.0, expected, &yaml);

            let opts = serde_saphyr::options! {
                reject_non_finite_typeless_float: true,
            };
            let err = serde_saphyr::from_str_with_options::<Value>(&yaml, opts).unwrap_err();
            assert!(
                matches!(
                    err.without_snippet(),
                    serde_saphyr::Error::NonFiniteFloat { value, .. } if value == literal
                ),
                "yaml: {yaml}, error: {err}"
            );
        }
    }
}

#[test]
fn reject_option_leaves_finite_numbers_and_strings_alone() {
    for reject in [false, true] {
        let opts = serde_saphyr::options! {
            reject_non_finite_typeless_float: reject,
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
fn default_preserves_non_finite_floats_in_untagged_enums() {
    for &(yaml, expected) in NON_FINITE_CASES {
        let value: FloatOrString = serde_saphyr::from_str(yaml).unwrap();
        let FloatOrString::Float(value) = value else {
            panic!("expected a float for {yaml}, got {value:?}");
        };
        assert_float(value, expected, yaml);
    }

    let value: FloatOrString = serde_saphyr::from_str("'.nan'").unwrap();
    assert!(matches!(value, FloatOrString::String(value) if value == ".nan"));
}

#[test]
fn default_preserves_non_finite_floats_in_flattened_maps() {
    #[derive(Debug, Deserialize)]
    struct Flattened {
        #[serde(flatten)]
        values: BTreeMap<String, f64>,
    }

    let value: Flattened =
        serde_saphyr::from_str("nan: .nan\npositive: .inf\nnegative: -.inf\n").unwrap();
    assert!(value.values["nan"].is_nan());
    assert_eq!(value.values["positive"], f64::INFINITY);
    assert_eq!(value.values["negative"], f64::NEG_INFINITY);
}

#[test]
fn default_preserves_non_finite_aliases_from_reader_and_slice() {
    let yaml = b"[&value .nan, *value, &negative -.inf, *negative]";
    let reader_values: Vec<AnyFloat> = serde_saphyr::from_reader(&yaml[..]).unwrap();
    let slice_values: Vec<AnyFloat> = serde_saphyr::from_slice(yaml).unwrap();
    for values in [reader_values, slice_values] {
        assert_eq!(values.len(), 4);
        assert!(values[0].0.is_nan());
        assert!(values[1].0.is_nan());
        assert_eq!(values[2].0, f64::NEG_INFINITY);
        assert_eq!(values[3].0, f64::NEG_INFINITY);
    }
}

#[test]
fn reject_option_does_not_affect_concrete_float_targets() {
    // This option governs deserialize_any. Concrete float targets continue to
    // accept YAML non-finite spellings and reject overflowing decimal literals.
    for reject in [false, true] {
        let opts = serde_saphyr::options! {
            reject_non_finite_typeless_float: reject,
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
fn reject_option_reports_raw_interpolated_scalar() {
    let mut props = HashMap::new();
    props.insert("TIMEOUT".to_string(), "1e999".to_string());

    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    }
    .with_properties(props);

    let err = serde_saphyr::from_str_with_options::<Value>("${TIMEOUT}\n", opts)
        .expect_err("resolved overflowing literal should error");

    let msg = err.to_string();
    assert!(msg.contains("${TIMEOUT}"), "raw scalar missing: {msg}");
    assert!(!msg.contains("1e999"), "resolved value leaked: {msg}");
}
