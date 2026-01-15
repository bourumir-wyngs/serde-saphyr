use serde::Deserialize;
use serde_saphyr::{Options, from_str_with_options};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, PartialEq)]
struct AsString {
    s: String,
}

#[test]
fn no_schema_off_allows_plain_numeric_into_string() {
    let y = "s: 123\n";
    let opts = Options {
        no_schema: false,
        ..Default::default()
    };
    let v: AsString =
        from_str_with_options(y, opts).expect("no_schema=false should accept plain 123 as string");
    assert_eq!(v.s, "123");
}

#[test]
fn no_schema_on_rejects_plain_numeric_into_string() {
    let y = "s: 123\n";
    let opts = Options {
        no_schema: true,
        ..Default::default()
    };
    let err = from_str_with_options::<AsString>(y, opts).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("must be quoted"), "unexpected error: {msg}");
}

#[test]
fn no_schema_on_accepts_quoted_numeric_into_string() {
    let y = "s: '123'\n";
    let opts = Options {
        no_schema: true,
        ..Default::default()
    };
    let v: AsString = from_str_with_options(y, opts)
        .expect("quoted numeric should be accepted as string when no_schema");
    assert_eq!(v.s, "123");
}

#[test]
fn no_schema_on_accepts_explicit_str_tag_into_string() {
    let y = "s: !!str 123\n";
    let opts = Options {
        no_schema: true,
        ..Default::default()
    };
    let v: AsString = from_str_with_options(y, opts)
        .expect("!!str 123 should be accepted as string when no_schema");
    assert_eq!(v.s, "123");
}

#[test]
fn no_schema_for_map_keys_string() {
    // Plain numeric key should be rejected when no_schema = true
    let y_plain = "1: a\n";
    let opts = Options {
        no_schema: true,
        ..Default::default()
    };
    let err = from_str_with_options::<BTreeMap<String, String>>(y_plain, opts.clone()).unwrap_err();
    assert!(
        err.to_string().contains("must be quoted"),
        "unexpected: {err}"
    );

    // Quoted numeric key should be accepted
    let y_quoted = "'1': a\n";
    let m = from_str_with_options::<BTreeMap<String, String>>(y_quoted, opts)
        .expect("quoted key should pass");
    assert_eq!(m.get("1").unwrap(), "a");
}

#[derive(Debug, Deserialize, PartialEq)]
struct HasChar {
    c: char,
}

#[test]
fn no_schema_on_char_rejects_plain_numeric_or_bool_like() {
    // Plain digit: looks like number, must be quoted when no_schema=true
    let y1 = "c: 1\n";
    let opts = Options {
        no_schema: true,
        ..Default::default()
    };
    let e1 = from_str_with_options::<HasChar>(y1, opts.clone()).unwrap_err();
    assert!(
        e1.to_string().contains("must be quoted"),
        "unexpected: {e1}"
    );

    // Quoted single digit is fine
    let y2 = "c: '1'\n";
    let v2: HasChar = from_str_with_options(y2, opts.clone()).expect("quoted char should pass");
    assert_eq!(v2.c, '1');

    // Plain boolean-like word: looks like bool, must be quoted
    let y3 = "c: true\n";
    let e3 = from_str_with_options::<HasChar>(y3, opts).unwrap_err();
    assert!(
        e3.to_string().contains("must be quoted"),
        "unexpected: {e3}"
    );
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, PartialEq)]
enum UnitE {
    #[serde(rename = "true")]
    True,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "ok")]
    Ok,
}

#[test]
fn no_schema_for_enum_unit_variant_name() {
    // no_schema=false accepts plain variant names even if they look like non-strings
    let off = Options {
        no_schema: false,
        ..Default::default()
    };
    let e_true: UnitE = from_str_with_options("true\n", off.clone())
        .expect("true as unit variant should parse with no_schema=false");
    assert!(matches!(e_true, UnitE::True));
    let e_one: UnitE = from_str_with_options("1\n", off)
        .expect("1 as unit variant should parse with no_schema=false");
    assert!(matches!(e_one, UnitE::One));

    // no_schema=true rejects plain true/1
    let on = Options {
        no_schema: true,
        ..Default::default()
    };
    let err_true = from_str_with_options::<UnitE>("true\n", on.clone()).unwrap_err();
    assert!(
        err_true.to_string().contains("must be quoted"),
        "unexpected: {err_true}"
    );
    let err_one = from_str_with_options::<UnitE>("1\n", on.clone()).unwrap_err();
    assert!(
        err_one.to_string().contains("must be quoted"),
        "unexpected: {err_one}"
    );

    // Quoted works with no_schema=true
    let e_true_q: UnitE =
        from_str_with_options("'true'\n", on.clone()).expect("quoted true should work");
    assert!(matches!(e_true_q, UnitE::True));
    let e_one_q: UnitE = from_str_with_options("'1'\n", on).expect("quoted 1 should work");
    assert!(matches!(e_one_q, UnitE::One));
}

#[derive(Debug, Deserialize, PartialEq)]
enum NewtypeE {
    #[serde(rename = "true")]
    True(i32),
    #[serde(rename = "1")]
    One(i32),
}

#[test]
fn no_schema_for_enum_externally_tagged_map_form() {
    // Map form with unquoted key should be rejected when no_schema=true
    let opts = Options {
        no_schema: true,
        ..Default::default()
    };

    let err1 = from_str_with_options::<NewtypeE>("{ true: 5 }\n", opts.clone()).unwrap_err();
    assert!(
        err1.to_string().contains("must be quoted"),
        "unexpected: {err1}"
    );
    let err2 = from_str_with_options::<NewtypeE>("{ 1: 5 }\n", opts.clone()).unwrap_err();
    assert!(
        err2.to_string().contains("must be quoted"),
        "unexpected: {err2}"
    );

    // Quoted keys should succeed
    let v1: NewtypeE = from_str_with_options("{ 'true': 7 }\n", opts.clone())
        .expect("quoted true key should work");
    assert!(matches!(v1, NewtypeE::True(7)));
    let v2: NewtypeE =
        from_str_with_options("{ '1': 9 }\n", opts).expect("quoted 1 key should work");
    assert!(matches!(v2, NewtypeE::One(9)));
}
