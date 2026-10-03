#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;

#[test]
fn unknown_field_error_has_location_and_renders_snippet() {
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Cfg {
        a: i32,
    }

    let yaml = "a: 1\nb: 2\n";

    let err = serde_saphyr::from_str::<Cfg>(yaml).expect_err("must fail");
    let rendered = err.to_string();

    assert!(
        rendered.contains("unknown field"),
        "expected unknown-field error, got: {rendered}"
    );
    assert!(
        rendered.contains(" -->"),
        "expected snippet header, got: {rendered}"
    );
    assert!(
        rendered.contains('^'),
        "expected span marker in snippet, got: {rendered}"
    );
}

#[test]
fn plain_string_into_int_error_has_location_and_renders_snippet() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Cfg {
        a: i32,
    }

    // No explicit !!int tag: this is a plain string that fails integer parsing.
    let yaml = "a: not-an-int\n";

    let err = serde_saphyr::from_str::<Cfg>(yaml).expect_err("must fail");
    let rendered = err.to_string();

    assert!(
        rendered.contains("i32") || rendered.contains("integer") || rendered.contains("invalid"),
        "expected integer parse/type error, got: {rendered}"
    );
    assert!(
        rendered.contains(" -->"),
        "expected snippet header, got: {rendered}"
    );
    assert!(
        rendered.contains('^'),
        "expected span marker in snippet, got: {rendered}"
    );
}

/// A collection where a borrowed `&str` is expected (a value, or a complex mapping key read into
/// `serde_json::Value`) is reported as such, not as "unexpected end of input", like `String`.
#[test]
fn collection_into_str_reports_unexpected_node_not_eof() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Borrowed<'a> {
        name: &'a str,
    }

    for yaml in ["name: [a]\n", "name: {a: 1}\n", "name:\n  - a\n"] {
        let err = serde_saphyr::from_str::<Borrowed>(yaml).expect_err("must fail");
        assert!(
            matches!(
                err.without_snippet(),
                serde_saphyr::Error::Unexpected { .. }
            ),
            "{yaml:?}: {err}"
        );
    }
    for yaml in ["[a]: b\n", "{[a]: b}\n", "? {x: 1}\n: b\n"] {
        let err = serde_saphyr::from_str::<serde_json::Value>(yaml).expect_err("must fail");
        assert!(
            matches!(
                err.without_snippet(),
                serde_saphyr::Error::Unexpected { .. }
            ),
            "{yaml:?}: {err}"
        );
    }
}
