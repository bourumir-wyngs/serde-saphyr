use indoc::indoc;
use serde::Serialize;

use serde_saphyr::to_string;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename = "__yaml_anchor")]
struct YamlAnchorPayload(usize, &'static str);

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename = "__yaml_weak_anchor")]
struct YamlWeakAnchorPayload(usize, bool, &'static str);

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename = "__yaml_commented")]
struct YamlCommentedPayload(&'static str, &'static str);

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename = "__yaml_commented")]
struct YamlCommentedNonString(bool, &'static str);

#[test]
fn internal_yaml_anchor_tuple_struct_emits_anchor_then_alias() {
    // This directly targets the `__yaml_anchor` special-case tuple-struct handling in `ser.rs`,
    // exercising the internal `UsizeCapture` and the define-vs-alias branching.
    let v = vec![YamlAnchorPayload(123, "x"), YamlAnchorPayload(123, "y")];
    let yaml = to_string(&v).expect("serialize __yaml_anchor payload");

    // Be tolerant about whitespace/details; the key property is that the first element defines
    // an anchor and the second reuses it as an alias.
    assert!(
        yaml.contains("&a1"),
        "expected anchor definition, got: {yaml}"
    );
    assert!(yaml.contains("*a1"), "expected alias reuse, got: {yaml}");
}

#[test]
fn internal_yaml_weak_anchor_present_false_serializes_as_null() {
    // Exercises the `present == false` branch which emits `null` and skips the third field.
    let v = vec![YamlWeakAnchorPayload(55, false, "value_is_ignored")];
    let yaml = to_string(&v).expect("serialize __yaml_weak_anchor payload");

    assert_eq!(
        yaml,
        indoc! {"\
        - null
    "}
    );
}

#[test]
fn internal_yaml_weak_anchor_present_true_emits_anchor_then_alias() {
    // Exercises BoolCapture(true) + define-vs-alias branching for weak anchors.
    let v = vec![
        YamlWeakAnchorPayload(77, true, "x"),
        YamlWeakAnchorPayload(77, true, "y"),
    ];
    let yaml = to_string(&v).expect("serialize __yaml_weak_anchor present payload");

    assert!(
        yaml.contains("&a1"),
        "expected anchor definition, got: {yaml}"
    );
    assert!(yaml.contains("*a1"), "expected alias reuse, got: {yaml}");
}

#[test]
fn internal_yaml_commented_payload_appends_inline_comment_in_block_context() {
    // This targets the `__yaml_commented` path which captures a string via `StrCapture` and stages
    // it as an inline comment. Newlines should be sanitized to spaces.
    let yaml = to_string(&YamlCommentedPayload("hello\nworld", "x"))
        .expect("serialize __yaml_commented payload");

    assert_eq!(yaml, "x # hello world\n");
}

#[test]
fn internal_yaml_commented_payload_requires_string_comment() {
    // If the first field isn't a string, `StrCapture` should reject it.
    let err = to_string(&YamlCommentedNonString(true, "x"))
        .expect_err("expected error when comment isn't a string");

    let msg = err.to_string();
    assert!(
        msg.contains("missing string") || msg.contains("unexpected"),
        "unexpected error message: {msg}"
    );
}
