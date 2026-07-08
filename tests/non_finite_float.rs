#![cfg(feature = "deserialize")]
use serde_json::Value;
#[cfg(feature = "properties")]
use std::collections::HashMap;

#[test]
fn default_rejects_non_finite_typeless_floats() {
    for src in [".inf", ".nan", "-.inf", "1e999", "9e400"] {
        let err = serde_saphyr::from_str::<Value>(src)
            .expect_err("default should reject non-finite typeless float");
        let msg = err.to_string();
        assert!(
            msg.contains("non-finite float"),
            "unexpected error for {src:?}: {msg}"
        );
    }
}

#[test]
fn disabled_reject_non_finite_typeless_float_round_trips_non_finite_floats_as_strings() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: false,
    };
    let v: Value = serde_saphyr::from_str_with_options(".inf", opts.clone())
        .expect("disabled should accept .inf");
    assert_eq!(v, Value::String(".inf".to_owned()));

    let v: Value = serde_saphyr::from_str_with_options(".nan", opts.clone())
        .expect("disabled should accept .nan");
    assert_eq!(v, Value::String(".nan".to_owned()));

    let v: Value =
        serde_saphyr::from_str_with_options("-.inf", opts).expect("disabled should accept -.inf");
    assert_eq!(v, Value::String("-.inf".to_owned()));
}

#[test]
fn disabled_reject_non_finite_typeless_float_canonicalizes_overflowing_decimal_literals() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: false,
    };
    // Before this option existed, an overflowing literal like `9e400` fell through to the
    // generic string fallback and round-tripped verbatim ("9e400") instead of the canonical
    // `.inf`. This is now recognized as non-finite even when the flag is off.
    let v: Value = serde_saphyr::from_str_with_options("9e400", opts.clone())
        .expect("overflowing literal should parse");
    assert_eq!(v, Value::String(".inf".to_owned()));

    let v: Value = serde_saphyr::from_str_with_options("-1e999", opts)
        .expect("overflowing literal should parse");
    assert_eq!(v, Value::String("-.inf".to_owned()));
}

#[test]
fn reject_non_finite_typeless_float_rejects_nan_inf_and_overflow() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    };
    for src in ["x: .nan", "x: .inf", "x: -.inf", "x: 1e999", "x: 9e400"] {
        let r: Result<Value, _> = serde_saphyr::from_str_with_options(src, opts.clone());
        assert!(
            r.is_err(),
            "{src:?} must error under reject_non_finite_typeless_float"
        );
    }
}

#[test]
fn reject_non_finite_typeless_float_leaves_finite_numbers_and_strings_alone() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    };

    let v: Value = serde_saphyr::from_str_with_options("2.5", opts.clone()).expect("finite ok");
    assert_eq!(v, Value::from(2.5));

    // A hostname-shaped string must not be mistaken for an overflowing numeral, since it
    // doesn't start with a digit (or sign followed by a digit).
    let v: Value =
        serde_saphyr::from_str_with_options("inf.example.com", opts).expect("hostname ok");
    assert_eq!(v, Value::String("inf.example.com".to_owned()));
}

#[test]
fn reject_non_finite_typeless_float_does_not_affect_concrete_float_targets() {
    // Concrete f32/f64 targets still receive YAML non-finite spellings as actual
    // non-finite values. Overflowing decimal literals remain invalid for concrete
    // float targets.
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    };
    let v: f64 = serde_saphyr::from_str_with_options(".inf", opts).expect("concrete f64 ok");
    assert!(v.is_infinite() && v.is_sign_positive());
}

#[test]
fn reject_non_finite_typeless_float_does_not_accept_concrete_float_overflow() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    };
    let err = serde_saphyr::from_str_with_options::<f64>("1e999", opts)
        .expect_err("overflowing decimal literals remain invalid for concrete float targets");

    let msg = err.to_string();
    assert!(
        msg.contains("invalid floating point"),
        "unexpected error: {msg}"
    );
}

#[test]
fn reject_non_finite_typeless_float_message_mentions_the_offending_value() {
    let opts = serde_saphyr::options! {
        reject_non_finite_typeless_float: true,
    };
    let err = serde_saphyr::from_str_with_options::<Value>("1e999", opts)
        .expect_err("overflowing literal should error");
    let msg = err.to_string();
    assert!(msg.contains("1e999"), "unexpected error: {msg}");
}

#[cfg(feature = "properties")]
#[test]
fn reject_non_finite_typeless_float_reports_raw_interpolated_scalar() {
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
