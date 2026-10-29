#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(feature = "properties-custom-fmt")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{
    Error, FormatterError, PropertyCustomFormatter, PropertySyntax, from_str_with_options,
};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// `vault:NAME` is looked up in a fixed table, `vault:` with an unknown name fails,
/// anything else is wrapped in angle brackets so tests can see the formatter ran.
fn vault(input: Cow<'_, str>) -> Result<Cow<'_, str>, FormatterError> {
    let Some(name) = input.strip_prefix("vault:") else {
        return Ok(Cow::Owned(format!("<{input}>")));
    };
    match name {
        "user" => Ok(Cow::Borrowed("alice")),
        "port" => Ok(Cow::Borrowed("5432")),
        _ => Err(FormatterError::new(format!("no secret named `{name}`"))),
    }
}

fn options() -> serde_saphyr::Options {
    let mut options = serde_saphyr::options! {};
    options.property_syntax = PropertySyntax::Custom(PropertyCustomFormatter::new(vault));
    options
}

#[derive(Debug, Deserialize, PartialEq)]
struct Db {
    user: String,
    port: u16,
    hosts: Vec<String>,
}

#[test]
fn formats_values_of_nested_maps_and_lists() {
    let yaml = "user: vault:user\nport: vault:port\nhosts:\n  - a\n  - [b, c]\n";
    #[derive(Debug, Deserialize)]
    struct Raw {
        user: String,
        port: u16,
        hosts: Vec<serde_json::Value>,
    }
    let parsed: Raw = from_str_with_options(yaml, options()).unwrap();
    assert_eq!(parsed.user, "alice");
    assert_eq!(parsed.port, 5432);
    assert_eq!(
        parsed.hosts,
        vec![serde_json::json!("<a>"), serde_json::json!(["<b>", "<c>"])]
    );
}

#[rstest]
#[case::plain("v: hi\n", "<hi>")]
#[case::single_quoted("v: 'hi'\n", "<hi>")]
#[case::double_quoted("v: \"h\\ti\"\n", "<h\ti>")]
#[case::literal_block("v: |\n  a\n  b\n", "<a\nb\n>")]
#[case::folded_block("v: >-\n  a\n  b\n", "<a b>")]
#[case::dollar_is_not_special("v: $X ${Y}\n", "<$X ${Y}>")]
fn formats_every_scalar_style(#[case] yaml: &str, #[case] expected: &str) {
    let parsed: BTreeMap<String, String> = from_str_with_options(yaml, options()).unwrap();
    assert_eq!(parsed["v"], expected);
}

#[test]
fn keys_are_not_formatted() {
    let parsed: BTreeMap<String, BTreeMap<String, String>> =
        from_str_with_options("outer:\n  inner: x\n", options()).unwrap();
    assert_eq!(parsed["outer"]["inner"], "<x>");
}

#[test]
fn formatted_values_are_typed_after_formatting() {
    let parsed: Db =
        from_str_with_options("user: vault:user\nport: vault:port\nhosts: []\n", options())
            .unwrap();
    assert_eq!(
        parsed,
        Db {
            user: "alice".to_owned(),
            port: 5432,
            hosts: vec![],
        }
    );
}

fn formatter_error(err: &Error) -> (&str, &str, serde_saphyr::Location) {
    let inner = match err {
        Error::WithSnippet { error, .. } => error.as_ref(),
        other => other,
    };
    let Error::PropertyFormatterFailed {
        value,
        reason,
        location,
    } = inner
    else {
        panic!("unexpected error: {err:?}");
    };
    (value, reason, *location)
}

#[rstest]
#[case::top_level_value("user: vault:nope\nport: 1\nhosts: []\n", 1, 7)]
#[case::later_line("user: a\nport: vault:nope\nhosts: []\n", 2, 7)]
#[case::block_sequence_item("user: a\nport: 1\nhosts:\n  - ok\n  - vault:nope\n", 5, 5)]
#[case::flow_sequence_item("user: a\nport: 1\nhosts: [ok, vault:nope]\n", 3, 13)]
#[case::quoted("user: a\nport: 1\nhosts: ['vault:nope']\n", 3, 9)]
#[case::literal_block("user: |-\n  vault:nope\nport: 1\nhosts: []\n", 2, 3)]
fn formatter_error_points_at_the_scalar(
    #[case] yaml: &str,
    #[case] line: u64,
    #[case] column: u64,
) {
    let err = from_str_with_options::<serde_json::Value>(yaml, options()).unwrap_err();
    let (value, reason, location) = formatter_error(&err);
    assert_eq!(value, "vault:nope");
    assert_eq!(reason, "no secret named `nope`");
    assert_eq!(
        (location.line(), location.column()),
        (line, column),
        "{err}"
    );
}

#[test]
fn rendered_error_shows_message_and_snippet() {
    let err = from_str_with_options::<Db>("user: a\nport: vault:nope\nhosts: []\n", options())
        .unwrap_err();
    let rendered = err.to_string();
    assert!(
        rendered.contains("cannot format value `vault:nope`: no secret named `nope`"),
        "{rendered}"
    );
    assert!(rendered.contains("port: vault:nope"), "{rendered}");
}

/// Maps `blank` to `""`, `nil` to `~`, and leaves everything else as written.
fn nullable(input: Cow<'_, str>) -> Result<Cow<'_, str>, FormatterError> {
    match input.as_ref() {
        "blank" => Ok(Cow::Borrowed("")),
        "nil" => Ok(Cow::Borrowed("~")),
        _ => Ok(input),
    }
}

fn nullable_options() -> serde_saphyr::Options {
    let mut options = serde_saphyr::options! {};
    options.property_syntax = PropertySyntax::Custom(PropertyCustomFormatter::new(nullable));
    options
}

#[rstest]
#[case::formatted_to_empty_is_a_string("v: blank\n", Some(""))]
#[case::formatted_to_tilde_is_null("v: nil\n", None)]
#[case::literal_null_is_null("v: null\n", None)]
#[case::literal_empty_is_null("v:\n", None)]
#[case::quoted_empty_is_a_string("v: ''\n", Some(""))]
fn formatter_output_null_handling_for_option(#[case] yaml: &str, #[case] expected: Option<&str>) {
    let parsed: BTreeMap<String, Option<String>> =
        from_str_with_options(yaml, nullable_options()).unwrap();
    assert_eq!(parsed["v"].as_deref(), expected);
}

#[rstest]
#[case::formatted_to_empty_is_a_string("v: blank\n", serde_json::json!(""))]
#[case::formatted_to_tilde_is_null("v: nil\n", serde_json::Value::Null)]
#[case::literal_empty_is_null("v:\n", serde_json::Value::Null)]
fn formatter_output_null_handling_for_untyped(
    #[case] yaml: &str,
    #[case] expected: serde_json::Value,
) {
    let parsed: BTreeMap<String, serde_json::Value> =
        from_str_with_options(yaml, nullable_options()).unwrap();
    assert_eq!(parsed["v"], expected);
}
