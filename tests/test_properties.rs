#![cfg(feature = "properties")]

use serde::Deserialize;
use serde_saphyr::{from_str_with_options, Options};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Deserialize, PartialEq)]
struct ScalarConfig {
    value: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NumericConfig {
    value: usize,
    nearby: String,
}

fn property_options_with_map(map: Option<HashMap<String, String>>) -> Options {
    let mut options = Options::default();
    options.property_map = map.map(Rc::new);
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