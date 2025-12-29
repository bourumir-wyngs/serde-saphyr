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
    let opts = serde_saphyr::Options {
        strict_booleans: true,
        ..serde_saphyr::Options::default()
    };
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
    let opts = serde_saphyr::Options {
        strict_booleans: true,
        ..serde_saphyr::Options::default()
    };

    let v_true: Value =
        serde_saphyr::from_str_with_options("true", opts.clone()).expect("parse true");
    assert_eq!(v_true, Value::Bool(true));

    let v_yes: Value = serde_saphyr::from_str_with_options("yes", opts).expect("parse yes");
    assert_eq!(v_yes, Value::String("yes".to_owned()));
}
