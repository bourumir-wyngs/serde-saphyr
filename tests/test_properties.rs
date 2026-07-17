#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(feature = "properties")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{
    Options, PropertySyntax, from_multiple_with_options, from_reader_with_options,
    from_str_with_options,
};
use std::collections::HashMap;
#[cfg(feature = "validator")]
use validator::Validate;

fn assert_redacted_message(message: &str, placeholder: &str, secret: &str) {
    assert!(
        message.contains(placeholder),
        "placeholder `{placeholder}` missing from: {message}"
    );
    assert!(
        !message.contains(secret),
        "secret `{secret}` leaked in: {message}"
    );
}

#[derive(Debug, Deserialize, PartialEq)]
struct ScalarConfig {
    value: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NumericConfig {
    value: usize,
    nearby: String,
}

#[derive(Debug, Deserialize)]
struct RawInner {
    value: String,
}

#[derive(Debug)]
struct CheckedInner;

impl<'de> Deserialize<'de> for CheckedInner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawInner::deserialize(deserializer)?;
        Err(serde::de::Error::custom(format!(
            "bad value: {}",
            raw.value
        )))
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Outer {
    inner: CheckedInner,
}

#[derive(Debug)]
struct CheckedSeq;

impl<'de> Deserialize<'de> for CheckedSeq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        Err(serde::de::Error::custom(format!(
            "bad values: {}",
            values.join(",")
        )))
    }
}

#[derive(Debug)]
struct AnyChecked;

impl<'de> Deserialize<'de> for AnyChecked {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = AnyChecked;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("anything")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom(format!("bad value: {s}")))
            }

            fn visit_string<E>(self, s: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&s)
            }
        }

        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SeqOuter {
    list: CheckedSeq,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Wrap {
    Hex(CustomHexByte),
}

fn property_options_with_map(map: Option<HashMap<String, String>>) -> Options {
    match map {
        Some(map) => serde_saphyr::options! {}.with_properties(map),
        None => serde_saphyr::options! {},
    }
}

fn property_options_with_map_and_no_schema(map: Option<HashMap<String, String>>) -> Options {
    match map {
        Some(map) => serde_saphyr::options! { no_schema: true }.with_properties(map),
        None => serde_saphyr::options! { no_schema: true },
    }
}

#[test]
fn replaces_plain_property_reference_when_defined() {
    let mut properties = HashMap::new();
    properties.insert("REFERENCE".to_string(), "resolved-value".to_string());

    let parsed: ScalarConfig = from_str_with_options(
        "value: ${REFERENCE}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(
        parsed,
        ScalarConfig {
            value: "resolved-value".to_string(),
        }
    );
}

#[test]
fn quoted_property_reference_is_left_verbatim() {
    let options = property_options_with_map(Some(HashMap::new()));

    let parsed: ScalarConfig = from_str_with_options("value: \"${PROPERTY}\"\n", options).unwrap();

    assert_eq!(parsed.value, "${PROPERTY}");
}

#[test]
fn block_property_reference_is_left_verbatim() {
    let mut properties = HashMap::new();
    properties.insert("TOKEN".to_string(), "resolved-secret".to_string());

    let parsed: ScalarConfig = from_str_with_options(
        "value: |\n  ${TOKEN}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "${TOKEN}\n");
}

#[test]
fn dollar_escape_keeps_placeholder_literal() {
    let mut properties = HashMap::new();
    properties.insert("NAME".to_string(), "resolved".to_string());

    let parsed: ScalarConfig = from_str_with_options(
        "value: $${NAME}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "${NAME}");
}

#[test]
fn disabled_properties_leave_placeholder_verbatim_without_error() {
    let options = property_options_with_map(None);

    let parsed: ScalarConfig = from_str_with_options("value: ${PROPERTY}\n", options).unwrap();

    assert_eq!(parsed.value, "${PROPERTY}");
}

#[test]
fn bare_reference_resolves_explicitly_empty_value_without_default() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct OptValue {
        value: Option<String>,
    }

    let mut properties = HashMap::new();
    properties.insert("EMPTY".to_string(), String::new());

    let parsed: OptValue = from_str_with_options(
        "value: ${EMPTY}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, None);
}

#[test]
fn default_form_uses_default_when_property_is_explicitly_empty() {
    let mut properties = HashMap::new();
    properties.insert("EMPTY".to_string(), String::new());

    let parsed: ScalarConfig = from_str_with_options(
        "value: ${EMPTY:-fallback}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "fallback");
}

#[rstest]
#[case::braced("${MISSING}", PropertySyntax::Braced)]
#[case::unbraced("$MISSING", PropertySyntax::BracedOrBare)]
fn missing_property_is_an_error_when_properties_are_enabled(
    #[case] placeholder: &str,
    #[case] syntax: PropertySyntax,
) {
    let options =
        serde_saphyr::options! { property_syntax: syntax }.with_properties(HashMap::new());

    let err = from_str_with_options::<ScalarConfig>(&format!("value: {placeholder}\n"), options)
        .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::UnresolvedProperty { name, location } => {
            assert_eq!(name, "MISSING");
            assert_ne!(*location, serde_saphyr::Location::UNKNOWN);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }

    let err_str = err.to_string();
    assert!(
        err_str.contains("missing property `MISSING`"),
        "unexpected: {err_str}"
    );
    assert!(
        err_str.contains(&format!("value: {placeholder}")),
        "unexpected: {err_str}"
    );
}

#[test]
fn invalid_property_name_is_an_error_when_properties_are_enabled() {
    let err = from_str_with_options::<ScalarConfig>(
        "value: ${1abc}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::InvalidPropertyName { name, location } => {
            assert_eq!(name, "${1abc}");
            assert_ne!(*location, serde_saphyr::Location::UNKNOWN);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }

    let err_str = err.to_string();
    assert!(
        err_str.contains("Invalid name: '${1abc}'"),
        "unexpected: {err_str}"
    );
    assert!(err_str.contains("value: ${1abc}"), "unexpected: {err_str}");
}

#[test]
fn interpolation_works_after_non_ascii_text() {
    let mut properties = HashMap::new();
    properties.insert("NAME".to_string(), "world".to_string());

    let parsed: ScalarConfig = from_str_with_options(
        "value: h\u{e9} ${NAME}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "h\u{e9} world");
}

#[test]
fn unsupported_default_form_errors() {
    let err = from_str_with_options::<ScalarConfig>(
        "value: ${NAME:=required}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::InvalidPropertyName { name, .. } => {
            assert_eq!(name, "${NAME:=required}");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn bare_dash_form_uses_default_when_unset() {
    let parsed: ScalarConfig = from_str_with_options(
        "value: ${NAME-fallback}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap();

    assert_eq!(parsed.value, "fallback");
}

#[test]
fn bare_dash_form_passes_empty_value_through() {
    let mut properties = HashMap::new();
    properties.insert("NAME".to_string(), String::new());

    let parsed: ScalarConfig = from_str_with_options(
        "value: prefix-${NAME-fallback}-suffix\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "prefix--suffix");
}

#[test]
fn alternate_form_substitutes_when_set() {
    let mut properties = HashMap::new();
    properties.insert("FLAG".to_string(), "anything".to_string());

    let parsed: ScalarConfig = from_str_with_options(
        "value: ${FLAG+enabled}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "enabled");
}

#[test]
fn alternate_with_colon_skips_empty_value() {
    let mut properties = HashMap::new();
    properties.insert("FLAG".to_string(), String::new());

    let parsed: ScalarConfig = from_str_with_options(
        "value: ${FLAG:+enabled}tail\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap();
    assert_eq!(parsed.value, "tail");
}

#[rstest]
#[case::bare_reference_set_empty("${EMPTY}", &[("EMPTY", "")])]
#[case::alternate_unset("${FLAG+enabled}", &[])]
#[case::alternate_colon_unset("${FLAG:+enabled}", &[])]
#[case::alternate_colon_set_empty("${FLAG:+enabled}", &[("FLAG", "")])]
#[case::empty_default_for_unset("${MISSING-}", &[])]
#[case::empty_colon_default_for_unset("${MISSING:-}", &[])]
#[case::dash_passes_empty_value_through("${EMPTY-fallback}", &[("EMPTY", "")])]
fn whole_scalar_empty_interpolation_deserializes_as_empty_string(
    #[case] placeholder: &str,
    #[case] entries: &[(&str, &str)],
) {
    let properties: HashMap<String, String> = entries
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();

    let parsed: ScalarConfig = from_str_with_options(
        &format!("value: {placeholder}\n"),
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value, "");
}

#[test]
fn default_text_is_taken_literally() {
    let parsed: ScalarConfig = from_str_with_options(
        "value: ${NAME:-keep $$ as-is}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap();

    assert_eq!(parsed.value, "keep $$ as-is");
}

#[test]
fn whitespace_around_name_is_rejected() {
    let err = from_str_with_options::<ScalarConfig>(
        "value: ${NAME :- fallback}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::InvalidPropertyName { name, .. } => {
            assert_eq!(name, "${NAME :- fallback}");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn leading_space_in_name_is_rejected() {
    let err = from_str_with_options::<ScalarConfig>(
        "value: ${ NAME}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::InvalidPropertyName { name, .. } => {
            assert_eq!(name, "${ NAME}");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn first_closing_brace_ends_default_text() {
    let parsed: ScalarConfig = from_str_with_options(
        "value: ${NAME:-a}b}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap();

    assert_eq!(parsed.value, "ab}");
}

#[test]
fn error_snippet_keeps_property_names_and_hides_resolved_values() {
    let mut properties = HashMap::new();
    properties.insert("BAD".to_string(), "not-a-number".to_string());
    properties.insert("NEARBY".to_string(), "nearby-secret".to_string());

    let err = from_str_with_options::<NumericConfig>(
        "value: ${BAD}\nnearby: ${NEARBY}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    let err_str = err.to_string();
    assert!(err_str.contains("value: ${BAD}"), "unexpected: {err_str}");
    assert!(
        err_str.contains("nearby: ${NEARBY}"),
        "unexpected: {err_str}"
    );
    assert!(
        !err_str.contains("not-a-number"),
        "diagnostic leaked resolved failing property value: {err_str}"
    );
    assert!(
        !err_str.contains("nearby-secret"),
        "diagnostic leaked resolved nearby property value: {err_str}"
    );
}

#[derive(Debug, Deserialize, PartialEq)]
struct InterpolatedString {
    value: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct InterpolatedChar {
    value: char,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct OnlyField {
    known: String,
}

#[derive(Debug, Deserialize, PartialEq)]
enum TaggedEnum {
    Known(u8),
}

#[derive(Debug, Deserialize)]
enum ScalarMode {
    Known,
}

#[derive(Debug, PartialEq)]
struct HexByte(u8);

impl<'de> Deserialize<'de> for HexByte {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        u8::from_str_radix(&s, 16).map(HexByte).map_err(|_| {
            serde::de::Error::invalid_value(serde::de::Unexpected::Str(&s), &"two hex digits")
        })
    }
}

#[derive(Debug, PartialEq)]
struct CustomHexByte(u8);

impl<'de> Deserialize<'de> for CustomHexByte {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        u8::from_str_radix(&s, 16)
            .map(CustomHexByte)
            .map_err(|_| serde::de::Error::custom(format!("bad value: {s}")))
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct HexCfg {
    value: HexByte,
}

#[derive(Debug, Deserialize, PartialEq)]
struct CustomHexCfg {
    value: CustomHexByte,
}

#[rstest]
#[case::braced("${PORT}", PropertySyntax::Braced)]
#[case::unbraced("$PORT", PropertySyntax::BracedOrBare)]
fn quoting_required_error_redacts_interpolated_string_value(
    #[case] placeholder: &str,
    #[case] syntax: PropertySyntax,
) {
    let mut properties = HashMap::new();
    properties.insert("PORT".to_string(), "5432".to_string());

    let options = serde_saphyr::options! {
        no_schema: true,
        property_syntax: syntax,
    }
    .with_properties(properties);

    let err =
        from_str_with_options::<InterpolatedString>(&format!("value: {placeholder}\n"), options)
            .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("must be quoted"), "unexpected: {msg}");
    assert_redacted_message(&msg, placeholder, "5432");
    if syntax == PropertySyntax::BracedOrBare {
        assert!(!msg.contains("${PORT}"));
    }
}

#[test]
fn quoting_required_error_redacts_interpolated_char_value() {
    let mut properties = HashMap::new();
    properties.insert("FLAG".to_string(), "true".to_string());

    let err = from_str_with_options::<InterpolatedChar>(
        "value: ${FLAG}\n",
        property_options_with_map_and_no_schema(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("must be quoted"), "unexpected: {msg}");
    assert!(!msg.contains("true"), "secret leaked in error: {msg}");
    assert!(msg.contains("${FLAG}"), "raw placeholder missing: {msg}");
}

#[test]
fn externally_tagged_enum_keys_are_not_interpolated() {
    let mut properties = HashMap::new();
    properties.insert("MODE".to_string(), "Known".to_string());

    let err = from_str_with_options::<TaggedEnum>(
        "${MODE}: 1\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("${MODE}"), "raw enum key missing: {msg}");
    assert!(
        !msg.contains("unknown variant `Known`"),
        "enum key was interpolated: {msg}"
    );
}

#[test]
fn scalar_enum_variant_error_redacts_interpolated_value() {
    let mut properties = HashMap::new();
    properties.insert("MODE".to_string(), "secret-variant".to_string());

    let err = from_str_with_options::<ScalarMode>(
        "${MODE}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("secret-variant"), "secret leaked: {msg}");
    assert!(msg.contains("${MODE}"), "raw placeholder missing: {msg}");
}

#[rstest]
#[case::bare("${TOKEN}")]
#[case::default_if_unset("${TOKEN-fallback}")]
#[case::default_if_unset_or_empty("${TOKEN:-fallback}")]
#[case::error_if_unset("${TOKEN?must be set}")]
#[case::error_if_unset_or_empty("${TOKEN:?must be set}")]
#[case::alternate_if_set("${TOKEN+fallback-replacement}")]
#[case::alternate_if_set_and_nonempty("${TOKEN:+fallback-replacement}")]
fn with_snippet_output_redacts_resolved_token_values(#[case] placeholder: &str) {
    let mut properties = HashMap::new();
    properties.insert("TOKEN".to_string(), "super-secret-token".to_string());

    let yaml = format!("value: {placeholder}\nnearby: {placeholder}\n");
    let err =
        from_str_with_options::<NumericConfig>(&yaml, property_options_with_map(Some(properties)))
            .unwrap_err();

    assert!(
        matches!(err, serde_saphyr::Error::WithSnippet { .. }),
        "expected WithSnippet wrapper, got: {err:?}"
    );

    let msg = err.to_string();
    assert_redacted_message(&msg, placeholder, "super-secret-token");
}

#[rstest]
#[case::braced("${FIELD}", PropertySyntax::Braced)]
#[case::unbraced("$FIELD", PropertySyntax::BracedOrBare)]
fn unknown_field_error_redacts_interpolated_key(
    #[case] placeholder: &str,
    #[case] syntax: PropertySyntax,
) {
    let mut properties = HashMap::new();
    properties.insert("FIELD".to_string(), "secret-field".to_string());

    let options = serde_saphyr::options! {
        property_syntax: syntax,
    }
    .with_properties(properties);

    let err = from_str_with_options::<OnlyField>(&format!("{placeholder}: value\n"), options)
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains(&format!("unknown field `{placeholder}`")),
        "unexpected: {msg}"
    );
    assert!(
        !msg.contains("secret-field"),
        "secret leaked in error: {msg}"
    );
}

#[test]
fn serde_invalid_value_does_not_leak_interpolated_value() {
    let mut properties = HashMap::new();
    properties.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<HexCfg>(
        "value: ${BAD}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
    assert!(msg.contains("${BAD}"), "raw placeholder missing: {msg}");
}

#[test]
fn serde_custom_error_does_not_leak_interpolated_value() {
    let mut properties = HashMap::new();
    properties.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<CustomHexCfg>(
        "value: ${BAD}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
    assert!(
        !msg.contains("bad value: zz-secret"),
        "custom error leaked secret: {msg}"
    );
}

#[test]
fn top_level_custom_error_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err =
        from_str_with_options::<CustomHexByte>("${BAD}\n", property_options_with_map(Some(props)))
            .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
    assert!(
        msg.contains("${BAD}") || msg.contains("invalid interpolated"),
        "unexpected: {msg}"
    );
}

#[test]
fn nested_container_custom_error_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<Outer>(
        "inner:\n  value: ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
    assert!(
        msg.contains("${BAD}") || msg.contains("invalid interpolated"),
        "unexpected: {msg}"
    );
}

#[test]
fn sequence_parent_custom_error_does_not_leak_interpolated_child_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<SeqOuter>(
        "list:\n  - ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
    assert!(
        msg.contains("${BAD}") || msg.contains("invalid interpolated"),
        "unexpected: {msg}"
    );
}

#[test]
fn from_reader_top_level_custom_error_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let reader = std::io::Cursor::new("${BAD}\n".as_bytes());
    let err = from_reader_with_options::<_, CustomHexByte>(
        reader,
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[test]
fn from_multiple_top_level_custom_error_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_multiple_with_options::<CustomHexByte>(
        "${BAD}\n---\n01\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[cfg(feature = "garde")]
mod garde_streaming_redaction {
    use super::*;
    use garde::Validate;

    fn reject_with_echo(value: &str, _ctx: &()) -> garde::Result {
        Err(garde::Error::new(format!("bad value: {value}")))
    }

    #[derive(Debug, Deserialize, Validate)]
    #[allow(dead_code)]
    struct GardeTopLevelSecret(#[garde(custom(reject_with_echo))] String);

    #[test]
    fn read_valid_top_level_validation_does_not_leak_interpolated_value() {
        let mut props = HashMap::new();
        props.insert("BAD".to_string(), "zz-secret".to_string());

        let mut reader = std::io::Cursor::new("${BAD}\n".as_bytes());
        let mut iter = serde_saphyr::read_with_options_valid::<_, GardeTopLevelSecret>(
            &mut reader,
            property_options_with_map(Some(props)),
        );

        let err = iter
            .next()
            .expect("iterator must yield validation result")
            .expect_err("validation must fail");
        let msg = err.to_string();

        assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
        assert!(
            msg.contains("${BAD}") || msg.contains("invalid interpolated"),
            "expected redacted value in message, got: {msg}"
        );
        assert!(iter.next().is_none(), "iterator must stop at end of input");
    }
}

#[cfg(feature = "validator")]
fn reject_with_echo(v: &str) -> Result<(), validator::ValidationError> {
    let mut err = validator::ValidationError::new("bad_secret");
    err.message = Some(format!("bad value: {v}").into());
    Err(err)
}

#[cfg(feature = "validator")]
fn reject_with_echo_code(v: &str) -> Result<(), validator::ValidationError> {
    let mut err = validator::ValidationError::new("bad");
    err.code = std::borrow::Cow::Owned(format!("bad-{v}"));
    Err(err)
}

#[cfg(feature = "validator")]
fn reject_with_echo_param_key(v: &str) -> Result<(), validator::ValidationError> {
    let mut err = validator::ValidationError::new("bad");
    err.add_param(std::borrow::Cow::Owned(format!("k-{v}")), &"x");
    Err(err)
}

#[cfg(feature = "validator")]
#[derive(Debug, Deserialize, Validate)]
struct ValidatorSecretCfg {
    #[validate(custom(function = "reject_with_echo"))]
    value: String,
}

#[cfg(feature = "validator")]
#[derive(Debug, Deserialize, Validate)]
struct ValidatorCodeCfg {
    #[validate(custom(function = "reject_with_echo_code"))]
    value: String,
}

#[cfg(feature = "validator")]
#[derive(Debug, Deserialize, Validate)]
struct ValidatorParamKeyCfg {
    #[validate(custom(function = "reject_with_echo_param_key"))]
    value: String,
}

#[cfg(feature = "validator")]
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ValidatorTopLevelSecret(String);

#[cfg(feature = "validator")]
impl Validate for ValidatorTopLevelSecret {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        let mut errors = validator::ValidationErrors::new();
        errors.add(
            "value",
            reject_with_echo(&self.0).expect_err("test validator must reject"),
        );
        Err(errors)
    }
}

#[cfg(feature = "validator")]
#[test]
fn validator_custom_message_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = serde_saphyr::from_str_with_options_validate::<ValidatorSecretCfg>(
        "value: ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[cfg(feature = "validator")]
#[test]
fn read_validate_top_level_validation_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let mut reader = std::io::Cursor::new("${BAD}\n".as_bytes());
    let mut iter = serde_saphyr::read_with_options_validate::<_, ValidatorTopLevelSecret>(
        &mut reader,
        property_options_with_map(Some(props)),
    );

    let err = iter
        .next()
        .expect("iterator must yield validation result")
        .expect_err("validation must fail");
    let msg = err.to_string();

    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
    assert!(
        msg.contains("${BAD}") || msg.contains("invalid interpolated"),
        "expected redacted value in message, got: {msg}"
    );
    assert!(iter.next().is_none(), "iterator must stop at end of input");
}

#[cfg(feature = "validator")]
#[test]
fn validator_dynamic_code_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = serde_saphyr::from_str_with_options_validate::<ValidatorCodeCfg>(
        "value: ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[cfg(feature = "validator")]
#[test]
fn validator_dynamic_param_key_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = serde_saphyr::from_str_with_options_validate::<ValidatorParamKeyCfg>(
        "value: ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[test]
fn top_level_deserialize_any_custom_error_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err =
        from_str_with_options::<AnyChecked>("${BAD}\n", property_options_with_map(Some(props)))
            .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[test]
fn overlapping_resolved_values_redact_longest_first() {
    let mut props = HashMap::new();
    props.insert("SHORT".to_string(), "ab".to_string());
    props.insert("LONG".to_string(), "abc".to_string());

    let err = from_str_with_options::<Outer>(
        "inner:\n  value: ${LONG}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("abc"), "secret leaked: {msg}");
    assert!(!msg.contains("ab"), "short secret leaked: {msg}");
}

#[test]
fn empty_resolved_value_does_not_expand_error_text() {
    let mut props = HashMap::new();
    props.insert("EMPTY".to_string(), "".to_string());

    let err = from_str_with_options::<CustomHexByte>(
        "${EMPTY}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.len() < 300, "message exploded: {msg}");
}

#[test]
fn enum_newtype_payload_map_form_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err =
        from_str_with_options::<Wrap>("Hex: ${BAD}\n", property_options_with_map(Some(props)))
            .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[test]
fn enum_newtype_payload_tagged_form_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err =
        from_str_with_options::<Wrap>("!Hex ${BAD}\n", property_options_with_map(Some(props)))
            .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[cfg(feature = "include")]
mod include_tests {
    use super::{ScalarConfig, property_options_with_map};
    use serde::Deserialize;
    use serde_saphyr::{
        IncludeRequest, IncludeResolveError, InputSource, ResolvedInclude, from_str_with_options,
    };
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, PartialEq)]
    struct RootConfig {
        cfg: ScalarConfig,
    }

    #[test]
    fn properties_are_resolved_inside_included_content() {
        let mut properties = HashMap::new();
        properties.insert("INCLUDED".to_string(), "from-include".to_string());

        let options = property_options_with_map(Some(properties)).with_include_resolver(
            |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
                if req.spec == "child.yaml" {
                    Ok(ResolvedInclude::new(
                        "child.yaml",
                        "child.yaml",
                        InputSource::from_string("value: ${INCLUDED}\n".to_string()),
                    ))
                } else {
                    Err(IncludeResolveError::Message(format!(
                        "file not found: {}",
                        req.spec
                    )))
                }
            },
        );

        let parsed: RootConfig =
            from_str_with_options("cfg: !include child.yaml\n", options).unwrap();

        assert_eq!(
            parsed,
            RootConfig {
                cfg: ScalarConfig {
                    value: "from-include".to_string(),
                },
            }
        );
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct OptionalScalarConfig {
    value: Option<String>,
}

// With the current code, interpolated nullish values are deserialized into
// None when target is an Option.
#[rstest]
#[case::empty_property("${EMPTY}", &[("EMPTY", "")], None)]
#[case::empty_default("${MISSING-}", &[], None)]
#[case::literal_null_default("${MISSING-null}", &[], None)]
#[case::literal_tilde_default("${MISSING-~}", &[], None)]
fn whole_scalar_nullish_interpolation_deserializes_as_some_string(
    #[case] placeholder: &str,
    #[case] entries: &[(&str, &str)],
    #[case] expected: Option<&str>,
) {
    let properties: HashMap<String, String> = entries
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();

    let parsed: OptionalScalarConfig = from_str_with_options(
        &format!("value: {placeholder}\n"),
        property_options_with_map(Some(properties)),
    )
    .unwrap();

    assert_eq!(parsed.value.as_deref(), expected);
}

#[rstest]
#[case::braced_only_keeps_dollar_literal("value: $TOKEN\n", PropertySyntax::Braced, "$TOKEN")]
#[case::resolves_bare_dollar_name(
    "value: $TOKEN\n",
    PropertySyntax::BracedOrBare,
    "resolved-value"
)]
#[case::still_resolves_braced_form(
    "value: ${TOKEN}\n",
    PropertySyntax::BracedOrBare,
    "resolved-value"
)]
#[case::quoted_scalar_stays_literal("value: \"$TOKEN\"\n", PropertySyntax::BracedOrBare, "$TOKEN")]
#[case::double_dollar_still_escapes("value: $$TOKEN\n", PropertySyntax::BracedOrBare, "$TOKEN")]
#[case::non_name_follower_stays_literal(
    "value: cost is $100 today\n",
    PropertySyntax::BracedOrBare,
    "cost is $100 today"
)]
#[case::brace_default_body_unchanged(
    "value: ${MISSING-$TOKEN}\n",
    PropertySyntax::BracedOrBare,
    "$TOKEN"
)]
fn property_syntax_scalar_deserialization(
    #[case] yaml: &str,
    #[case] syntax: PropertySyntax,
    #[case] expected: &str,
) {
    let properties = HashMap::from([("TOKEN".to_string(), "resolved-value".to_string())]);

    let options = serde_saphyr::options! { property_syntax: syntax }.with_properties(properties);

    let parsed: ScalarConfig = from_str_with_options(yaml, options).unwrap();

    assert_eq!(parsed.value, expected);
}
