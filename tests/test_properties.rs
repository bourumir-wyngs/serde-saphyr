#![cfg(feature = "properties")]

use serde::Deserialize;
use serde_saphyr::{from_str_with_options, Options};
use std::collections::HashMap;
use std::rc::Rc;

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
        Err(serde::de::Error::custom(format!("bad value: {}", raw.value)))
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Outer {
    inner: CheckedInner,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Wrap {
    Hex(CustomHexByte),
}

fn property_options_with_map(map: Option<HashMap<String, String>>) -> Options {
    Options {
        property_map: map.map(Rc::new),
        ..Options::default()
    }
}

fn property_options_with_map_and_no_schema(map: Option<HashMap<String, String>>) -> Options {
    let mut options = property_options_with_map(map);
    options.no_schema = true;
    options
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

    let parsed: ScalarConfig =
        from_str_with_options("value: \"${PROPERTY}\"\n", options).unwrap();

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
fn missing_property_is_an_error_when_properties_are_enabled() {
    let err = from_str_with_options::<ScalarConfig>(
        "value: ${MISSING}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::UnresolvedProperty { name, location } => {
            assert_eq!(name, "MISSING");
            assert_ne!(*location, serde_saphyr::Location::UNKNOWN);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }

    let err_str = err.to_string();
    assert!(err_str.contains("missing property `MISSING`"), "unexpected: {err_str}");
    assert!(err_str.contains("value: ${MISSING}"), "unexpected: {err_str}");
}

#[test]
fn invalid_property_name_is_an_error_when_properties_are_enabled() {
    let err = from_str_with_options::<ScalarConfig>(
        "value: ${ab-cd}\n",
        property_options_with_map(Some(HashMap::new())),
    )
    .unwrap_err();

    match err.without_snippet() {
        serde_saphyr::Error::InvalidPropertyName { name, location } => {
            assert_eq!(name, "${ab-cd}");
            assert_ne!(*location, serde_saphyr::Location::UNKNOWN);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }

    let err_str = err.to_string();
    assert!(err_str.contains("Invalid name: '${ab-cd}'"), "unexpected: {err_str}");
    assert!(err_str.contains("value: ${ab-cd}"), "unexpected: {err_str}");
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
    assert!(err_str.contains("nearby: ${NEARBY}"), "unexpected: {err_str}");
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
        u8::from_str_radix(&s, 16)
            .map(HexByte)
            .map_err(|_| serde::de::Error::invalid_value(serde::de::Unexpected::Str(&s), &"two hex digits"))
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

#[test]
fn quoting_required_error_redacts_interpolated_string_value() {
    let mut properties = HashMap::new();
    properties.insert("PORT".to_string(), "5432".to_string());

    let err = from_str_with_options::<InterpolatedString>(
        "value: ${PORT}\n",
        property_options_with_map_and_no_schema(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("must be quoted"), "unexpected: {msg}");
    assert_redacted_message(&msg, "${PORT}", "5432");
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
    assert!(!msg.contains("unknown variant `Known`"), "enum key was interpolated: {msg}");
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

#[test]
fn with_snippet_output_redacts_resolved_token_values() {
    let mut properties = HashMap::new();
    properties.insert("TOKEN".to_string(), "super-secret-token".to_string());

    let err = from_str_with_options::<NumericConfig>(
        "value: ${TOKEN}\nnearby: ${TOKEN}\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    assert!(
        matches!(err, serde_saphyr::Error::WithSnippet { .. }),
        "expected WithSnippet wrapper, got: {err:?}"
    );

    let msg = err.to_string();
    assert_redacted_message(&msg, "${TOKEN}", "super-secret-token");
}

#[test]
fn unknown_field_error_redacts_interpolated_key() {
    let mut properties = HashMap::new();
    properties.insert("FIELD".to_string(), "secret-field".to_string());

    let err = from_str_with_options::<OnlyField>(
        "${FIELD}: value\n",
        property_options_with_map(Some(properties)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("unknown field `${FIELD}`"), "unexpected: {msg}");
    assert!(!msg.contains("secret-field"), "secret leaked in error: {msg}");
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
    assert!(!msg.contains("bad value: zz-secret"), "custom error leaked secret: {msg}");
}

#[test]
fn top_level_custom_error_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<CustomHexByte>(
        "${BAD}\n",
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
fn enum_newtype_payload_map_form_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<Wrap>(
        "Hex: ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[test]
fn enum_newtype_payload_tagged_form_does_not_leak_interpolated_value() {
    let mut props = HashMap::new();
    props.insert("BAD".to_string(), "zz-secret".to_string());

    let err = from_str_with_options::<Wrap>(
        "!Hex ${BAD}\n",
        property_options_with_map(Some(props)),
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(!msg.contains("zz-secret"), "secret leaked: {msg}");
}

#[cfg(feature = "include")]
mod include_tests {
    use super::{property_options_with_map, ScalarConfig};
    use serde::Deserialize;
    use serde_saphyr::{
        from_str_with_options, IncludeRequest, IncludeResolveError, InputSource, ResolvedInclude,
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
                    Ok(ResolvedInclude {
                        id: "child.yaml".to_string(),
                        name: "child.yaml".to_string(),
                        source: InputSource::from_string("value: ${INCLUDED}\n".to_string()),
                    })
                } else {
                    Err(IncludeResolveError::Message(format!(
                        "file not found: {}",
                        req.spec
                    )))
                }
            },
        );

        let parsed: RootConfig = from_str_with_options("cfg: !include child.yaml\n", options).unwrap();

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