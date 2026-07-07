#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde_json::Value;

#[test]
fn default_round_trips_non_finite_floats_as_strings() {
    let v: Value = serde_saphyr::from_str(".inf").expect("default should accept .inf");
    assert_eq!(v, Value::String(".inf".to_owned()));

    let v: Value = serde_saphyr::from_str(".nan").expect("default should accept .nan");
    assert_eq!(v, Value::String(".nan".to_owned()));

    let v: Value = serde_saphyr::from_str("-.inf").expect("default should accept -.inf");
    assert_eq!(v, Value::String("-.inf".to_owned()));
}

#[test]
fn default_canonicalizes_overflowing_decimal_literals() {
    // Before this option existed, an overflowing literal like `9e400` fell through to the
    // generic string fallback and round-tripped verbatim ("9e400") instead of the canonical
    // `.inf`. This is now recognized as non-finite even when the flag is off.
    let v: Value = serde_saphyr::from_str("9e400").expect("overflowing literal should parse");
    assert_eq!(v, Value::String(".inf".to_owned()));

    let v: Value = serde_saphyr::from_str("-1e999").expect("overflowing literal should parse");
    assert_eq!(v, Value::String("-.inf".to_owned()));
}

#[test]
fn error_on_non_finite_float_rejects_nan_inf_and_overflow() {
    let opts = serde_saphyr::options! {
        error_on_non_finite_float: true,
    };
    for src in ["x: .nan", "x: .inf", "x: -.inf", "x: 1e999", "x: 9e400"] {
        let r: Result<Value, _> =
            serde_saphyr::from_str_with_options(src, opts.clone());
        assert!(r.is_err(), "{src:?} must error under error_on_non_finite_float");
    }
}

#[test]
fn error_on_non_finite_float_leaves_finite_numbers_and_strings_alone() {
    let opts = serde_saphyr::options! {
        error_on_non_finite_float: true,
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
fn error_on_non_finite_float_does_not_affect_concrete_float_targets() {
    // Concrete f32/f64 targets should still receive the actual non-finite value rather
    // than erroring, regardless of `error_on_non_finite_float` (which only governs the
    // typeless `deserialize_any` path).
    let opts = serde_saphyr::options! {
        error_on_non_finite_float: true,
    };
    let v: f64 = serde_saphyr::from_str_with_options(".inf", opts).expect("concrete f64 ok");
    assert!(v.is_infinite() && v.is_sign_positive());
}

#[test]
fn error_on_non_finite_float_message_mentions_the_offending_value() {
    let opts = serde_saphyr::options! {
        error_on_non_finite_float: true,
    };
    let err = serde_saphyr::from_str_with_options::<Value>("1e999", opts)
        .expect_err("overflowing literal should error");
    let msg = err.to_string();
    assert!(msg.contains("1e999"), "unexpected error: {msg}");
}
