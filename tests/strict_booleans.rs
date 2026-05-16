#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, PartialEq)]
struct Flag {
    enabled: bool,
}

#[test]
fn non_strict_accepts_yaml11_bool_literals() {
    let y = "enabled: yes\n";
    let got: Flag = serde_saphyr::from_str(y).expect("non-strict should accept yes");
    assert_eq!(got, Flag { enabled: true });
}

#[test]
fn strict_rejects_yaml11_bool_literals() {
    let y = "enabled: yes\n";
    let opts = serde_saphyr::options! { strict_booleans: true };
    let err =
        serde_saphyr::from_str_with_options::<Flag>(y, opts).expect_err("strict should reject yes");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid boolean") || msg.contains("strict"),
        "unexpected error: {}",
        msg
    );
}

#[test]
fn strict_inference_for_json_value() {
    // In strict mode, only true/false are booleans; `yes` should remain a string when inferring into Value
    let opts = serde_saphyr::options! { strict_booleans: true };

    let v_true: Value =
        serde_saphyr::from_str_with_options("true", opts.clone()).expect("parse true");
    assert_eq!(v_true, Value::Bool(true));

    let v_yes: Value = serde_saphyr::from_str_with_options("yes", opts).expect("parse yes");
    assert_eq!(v_yes, Value::String("yes".to_owned()));
}

#[derive(Debug, Deserialize, PartialEq)]
struct StrictBoolKey {
    y: u8,
    yes: u8,
    on: u8,
    enabled: u8,
}

#[test]
fn strict_booleans_with_no_schema_should_not_require_quoting_y_key() {
    // Bug: even with strict_booleans: true (which should treat YAML 1.1
    // boolean literals like "y" as plain strings), combining it with
    // no_schema: true incorrectly demands that the map key "y" be quoted.
    let opts = serde_saphyr::options! {
        strict_booleans: true,
        no_schema: true,
    };
    let yaml = r#"
        y: 1
        yes: 2
        on: 3
        enabled: 4
"#;
    let got: StrictBoolKey = serde_saphyr::from_str_with_options(yaml, opts)
        .expect("strict_booleans + no_schema should accept bare YAML 1.1 boolean literals as keys");
    assert_eq!(
        got,
        StrictBoolKey {
            y: 1,
            yes: 2,
            on: 3,
            enabled: 4
        }
    );
}
