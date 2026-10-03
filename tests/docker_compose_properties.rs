#![cfg(feature = "properties")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{
    Error, Location, Options, PropertySyntax, budget::BudgetBreach, from_reader_with_options,
    from_str_with_options,
};
use std::collections::HashMap;

fn compose_options(entries: &[(&str, &str)]) -> Options {
    serde_saphyr::options! {
        property_syntax: PropertySyntax::DockerCompose,
    }
    .with_properties(
        entries
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
    )
}

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    value: String,
}

#[rstest]
#[case::plain("value: before-$SET-${SET}\n", "before-value-value")]
#[case::single_quoted("value: 'before-$SET-${SET}'\n", "before-value-value")]
#[case::double_quoted("value: \"before-$SET-${SET}\"\n", "before-value-value")]
#[case::literal("value: |\n  before-$SET\n  ${SET}\n", "before-value\nvalue\n")]
#[case::folded("value: >-\n  before-$SET\n  ${SET}\n", "before-value value")]
fn interpolates_all_yaml_string_styles(#[case] yaml: &str, #[case] expected: &str) {
    let options = compose_options(&[("SET", "value")]);
    let parsed: Config = from_str_with_options(yaml, options.clone()).unwrap();
    assert_eq!(parsed.value, expected);
    let parsed: Config = from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(parsed.value, expected);
}

#[rstest]
#[case::braced_set("${SET}", "value")]
#[case::bare_set("$SET", "value")]
#[case::braced_empty("${EMPTY}", "")]
#[case::bare_empty("$EMPTY", "")]
#[case::default_unset("${MISSING-default}", "default")]
#[case::default_empty("${EMPTY-default}", "")]
#[case::default_set("${SET-default}", "value")]
#[case::default_nonempty_unset("${MISSING:-default}", "default")]
#[case::default_nonempty_empty("${EMPTY:-default}", "default")]
#[case::default_nonempty_set("${SET:-default}", "value")]
#[case::alternative_unset("${MISSING+replacement}", "")]
#[case::alternative_empty("${EMPTY+replacement}", "replacement")]
#[case::alternative_set("${SET+replacement}", "replacement")]
#[case::alternative_nonempty_unset("${MISSING:+replacement}", "")]
#[case::alternative_nonempty_empty("${EMPTY:+replacement}", "")]
#[case::alternative_nonempty_set("${SET:+replacement}", "replacement")]
#[case::required_empty("${EMPTY?required}", "")]
#[case::required_set("${SET?required}", "value")]
#[case::required_nonempty_set("${SET:?required}", "value")]
#[case::empty_default("${MISSING-}", "")]
#[case::empty_replacement("${SET:+}", "")]
fn operator_conditions(#[case] template: &str, #[case] expected: &str) {
    let parsed: Config = from_str_with_options(
        &format!("value: '{template}'\n"),
        compose_options(&[("SET", "value"), ("EMPTY", "")]),
    )
    .unwrap();
    assert_eq!(parsed.value, expected);
}

#[rstest]
#[case::required_unset("${MISSING?please configure it}", false)]
#[case::required_nonempty_unset("${MISSING:?please configure it}", false)]
#[case::required_nonempty_empty("${EMPTY:?please configure it}", true)]
fn required_values_report_errors(#[case] template: &str, #[case] empty: bool) {
    let err = from_str_with_options::<Config>(
        &format!("value: '{template}'\n"),
        compose_options(&[("EMPTY", "")]),
    )
    .unwrap_err();
    match err.without_snippet() {
        Error::PropertyRequiredButEmpty {
            name,
            message,
            location,
        } if empty => {
            assert_eq!(name, "EMPTY");
            assert_eq!(message, "please configure it");
            assert_ne!(*location, Location::UNKNOWN);
        }
        Error::PropertyRequiredButUnset {
            name,
            message,
            location,
        } if !empty => {
            assert_eq!(name, "MISSING");
            assert_eq!(message, "please configure it");
            assert_ne!(*location, Location::UNKNOWN);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

const HINT_SECRET: &str = "sensitive-property-value";
const FINAL_HINT_SECRET: &str = "sensitive-final-value ${UNRESOLVED} $$FINAL";

fn required_hint_options(syntax: PropertySyntax) -> Options {
    let mut options = compose_options(&[
        ("SECRET", HINT_SECRET),
        ("FINAL", FINAL_HINT_SECRET),
        ("SET", "condition-value"),
        ("EMPTY", ""),
        ("K", HINT_SECRET),
        ("ſ", HINT_SECRET),
    ]);
    options.property_syntax = syntax;
    options
}

fn assert_required_hint(err: &Error, expected_name: &str, expected_hint: &str) {
    match err.without_snippet() {
        Error::PropertyRequiredButUnset { name, message, .. } if expected_name != "EMPTY" => {
            assert_eq!(name, expected_name);
            assert_eq!(message, expected_hint);
        }
        Error::PropertyRequiredButEmpty { name, message, .. } if expected_name == "EMPTY" => {
            assert_eq!(name, expected_name);
            assert_eq!(message, expected_hint);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

fn assert_hint_secrets_are_absent(err: &Error) {
    for rendered in [err.to_string(), format!("{err:?}")] {
        assert!(!rendered.contains(HINT_SECRET), "{rendered}");
        assert!(!rendered.contains(FINAL_HINT_SECRET), "{rendered}");
    }
}

#[rstest]
#[case::unset(
    "${MISSING?prefix ${SECRET} suffix}",
    "MISSING",
    "prefix ${SECRET} suffix"
)]
#[case::unset_or_empty("${MISSING:?${SECRET}}", "MISSING", "${SECRET}")]
#[case::empty("${EMPTY:?${SECRET}}", "EMPTY", "${SECRET}")]
#[case::repeated("${MISSING:?${SECRET}/${SECRET}}", "MISSING", "${SECRET}/${SECRET}")]
#[case::nested_default("${MISSING:?${ABSENT:-${SECRET}}}", "MISSING", "${ABSENT:-${SECRET}}")]
#[case::nested_alternative("${MISSING:?${SET:+${SECRET}}}", "MISSING", "${SET:+${SECRET}}")]
#[case::set_default("${MISSING:?${SECRET:-unused}}", "MISSING", "${SECRET:-unused}")]
#[case::set_required("${MISSING:?${SECRET?unused}}", "MISSING", "${SECRET?unused}")]
#[case::inner_required(
    "${MISSING:?${OTHER?prefix ${SECRET} suffix}}",
    "OTHER",
    "prefix ${SECRET} suffix"
)]
#[case::final_property_text("${MISSING:?${FINAL}}", "MISSING", "${FINAL}")]
fn required_hints_preserve_source_when_property_values_are_used(
    #[case] template: &str,
    #[case] name: &str,
    #[case] expected: &str,
    #[values(
        PropertySyntax::Braced,
        PropertySyntax::BracedOrBare,
        PropertySyntax::DockerCompose
    )]
    syntax: PropertySyntax,
) {
    let options = required_hint_options(syntax);
    for err in [
        from_str_with_options::<String>(template, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(template.as_bytes(), options).unwrap_err(),
    ] {
        assert_required_hint(&err, name, expected);
        assert_hint_secrets_are_absent(&err);
    }
}

#[rstest]
#[case::plain("${MISSING:?prefix $SECRET suffix}", "prefix $SECRET suffix")]
#[case::single_quoted("'${MISSING:?$SECRET}'", "$SECRET")]
#[case::double_quoted("\"${MISSING:?$SECRET}\"", "$SECRET")]
#[case::literal("|-\n  ${MISSING:?$SECRET}\n", "$SECRET")]
#[case::folded(">-\n  ${MISSING:?$SECRET}\n", "$SECRET")]
#[case::unicode("'${MISSING:?$K/$ſ}'", "$K/$ſ")]
fn compose_required_hints_redact_bare_references_in_all_styles(
    #[case] yaml: &str,
    #[case] expected: &str,
) {
    let options = required_hint_options(PropertySyntax::DockerCompose);
    for err in [
        from_str_with_options::<String>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        assert_required_hint(&err, "MISSING", expected);
        assert_hint_secrets_are_absent(&err);
    }
}

#[rstest]
fn aliased_required_hints_do_not_expose_property_values(
    #[values(
        PropertySyntax::Braced,
        PropertySyntax::BracedOrBare,
        PropertySyntax::DockerCompose
    )]
    syntax: PropertySyntax,
) {
    let yaml = "&hint ${MISSING:?${SECRET}}: ignored\nvalue: *hint\n";
    let options = required_hint_options(syntax);
    for err in [
        from_str_with_options::<HashMap<String, String>>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, HashMap<String, String>>(yaml.as_bytes(), options)
            .unwrap_err(),
    ] {
        assert!(matches!(
            err.without_snippet(),
            Error::PropertyRequiredButUnset { .. } | Error::AliasError { .. }
        ));
        assert!(err.to_string().contains("${SECRET}"));
        assert_hint_secrets_are_absent(&err);
    }
}

#[rstest]
#[case::literal("${MISSING?please configure it}", "MISSING", "please configure it")]
#[case::public_default("${MISSING:?${ABSENT:-public default}}", "MISSING", "public default")]
#[case::public_alternative(
    "${MISSING:?${SECRET:+public replacement}}",
    "MISSING",
    "public replacement"
)]
#[case::empty_value("${MISSING:?before${EMPTY}after}", "MISSING", "beforeafter")]
#[case::empty_value_default("${MISSING:?${EMPTY:-public default}}", "MISSING", "public default")]
#[case::inactive_alternative("${MISSING:?${ABSENT:+${SECRET}}}", "MISSING", "")]
#[case::inner_public_error("${MISSING:?${SECRET}${OTHER?public hint}}", "OTHER", "public hint")]
#[case::literal_matching_property("${MISSING:?sensitive-property-value}", "MISSING", HINT_SECRET)]
fn required_hints_without_property_values_keep_evaluated_text(
    #[case] template: &str,
    #[case] name: &str,
    #[case] expected: &str,
    #[values(
        PropertySyntax::Braced,
        PropertySyntax::BracedOrBare,
        PropertySyntax::DockerCompose
    )]
    syntax: PropertySyntax,
) {
    let options = required_hint_options(syntax);
    for err in [
        from_str_with_options::<String>(template, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(template.as_bytes(), options).unwrap_err(),
    ] {
        assert_required_hint(&err, name, expected);
    }
}

#[rstest]
#[case::escape("${MISSING:?$$SECRET}", "$SECRET")]
#[case::missing_bare("${MISSING:?before $ABSENT after}", "before  after")]
#[case::missing_braced("${MISSING:?before ${ABSENT} after}", "before  after")]
fn compose_required_hints_evaluate_escapes_and_missing_references(
    #[case] template: &str,
    #[case] expected: &str,
) {
    let options = required_hint_options(PropertySyntax::DockerCompose);
    for err in [
        from_str_with_options::<String>(template, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(template.as_bytes(), options).unwrap_err(),
    ] {
        assert_required_hint(&err, "MISSING", expected);
        assert_hint_secrets_are_absent(&err);
    }
}

#[rstest]
#[case::default("${ABSENT:-${SECRET}}", HINT_SECRET)]
#[case::replacement("${SET:+${SECRET}}", HINT_SECRET)]
#[case::inactive_required_hint("${SECRET?${OTHER?unreachable}}", HINT_SECRET)]
#[case::final_property_text("${ABSENT:-${FINAL}}", FINAL_HINT_SECRET)]
fn successful_expansions_keep_property_values(
    #[case] template: &str,
    #[case] expected: &str,
    #[values(
        PropertySyntax::Braced,
        PropertySyntax::BracedOrBare,
        PropertySyntax::DockerCompose
    )]
    syntax: PropertySyntax,
) {
    let options = required_hint_options(syntax);
    for value in [
        from_str_with_options::<String>(template, options.clone()).unwrap(),
        from_reader_with_options::<_, String>(template.as_bytes(), options).unwrap(),
    ] {
        assert_eq!(value, expected);
    }
}

#[rstest]
#[case::malformed("${MISSING:?${SECRET}${}}", true)]
#[case::depth_limit("${MISSING:?${SECRET}}", false)]
fn required_hint_redaction_preserves_interpolation_failures(
    #[case] template: &str,
    #[case] malformed: bool,
    #[values(
        PropertySyntax::Braced,
        PropertySyntax::BracedOrBare,
        PropertySyntax::DockerCompose
    )]
    syntax: PropertySyntax,
) {
    let mut options = required_hint_options(syntax);
    if !malformed {
        options.budget = serde_saphyr::budget! { max_property_expansion_depth: 0 };
    }
    for err in [
        from_str_with_options::<String>(template, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(template.as_bytes(), options).unwrap_err(),
    ] {
        if malformed {
            assert!(matches!(
                err.without_snippet(),
                Error::InvalidPropertyName { .. }
            ));
        } else {
            assert!(matches!(
                err.without_snippet(),
                Error::Budget {
                    breach: BudgetBreach::PropertyExpansionDepth {
                        depth: 1,
                        max_depth: 0
                    },
                    ..
                }
            ));
        }
        assert_hint_secrets_are_absent(&err);
    }
}

#[rstest]
fn required_hint_redaction_preserves_work_budget(
    #[values(
        PropertySyntax::Braced,
        PropertySyntax::BracedOrBare,
        PropertySyntax::DockerCompose
    )]
    syntax: PropertySyntax,
) {
    let mut options = required_hint_options(syntax);
    options.budget = serde_saphyr::budget! { max_total_property_interpolation_work: 0 };
    let template = "${MISSING:?${SECRET}}";
    for err in [
        from_str_with_options::<String>(template, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(template.as_bytes(), options).unwrap_err(),
    ] {
        assert!(matches!(
            err.without_snippet(),
            Error::Budget {
                breach: BudgetBreach::PropertyInterpolationWork { .. },
                ..
            }
        ));
        assert_hint_secrets_are_absent(&err);
    }
}

#[rstest]
#[case::bare_default("${MISSING:-$SET}", "value")]
#[case::braced_default("${MISSING:-${SET}}", "value")]
#[case::nested_default("${MISSING:-${EMPTY:-$SET}}", "value")]
#[case::bare_replacement("${SET:+$SET}", "value")]
#[case::escaped_default("${MISSING:-$$SET}", "$SET")]
#[case::escaped_braced_default("${MISSING:-$${SET}}", "${SET}")]
#[case::mixed_default("${MISSING:-${SET}-$$SET-$SET}", "value-$SET-value")]
#[case::escaped_bare("$$SET", "$SET")]
#[case::escaped_braced("$${SET}", "${SET}")]
#[case::escape_then_reference("$$$SET", "$value")]
#[case::literal_dollars(
    "price $100; punctuation $!; trailing $",
    "price $100; punctuation $!; trailing $"
)]
#[case::brace_default("${MISSING:-{json}}", "{json}")]
#[case::unused_brace_default("${SET:-{json}}", "value")]
#[case::nested_literal_braces("${MISSING:-{{json}}}", "{{json}}")]
#[case::unbalanced_literal_default("${SET:-{json}", "value")]
#[case::unbalanced_selected_literal_default("${MISSING:-{json}", "{json")]
#[case::compose_brace_matching("${SET:-{{}}}", "value}")]
#[case::suffix_after_default("${SET:-{json}}suffix", "valuesuffix")]
fn recursively_expands_selected_operator_text(#[case] template: &str, #[case] expected: &str) {
    let parsed: Config = from_str_with_options(
        &format!("value: '{template}'\n"),
        compose_options(&[("SET", "value"), ("EMPTY", "")]),
    )
    .unwrap();
    assert_eq!(parsed.value, expected);
}

#[test]
fn property_values_are_final_and_names_are_case_sensitive() {
    let parsed: Config = from_str_with_options(
        "value: '$SET/$set/$_NAME9'\n",
        compose_options(&[
            ("SET", "${MISSING} $$OTHER"),
            ("set", "lower"),
            ("_NAME9", "last"),
        ]),
    )
    .unwrap();
    assert_eq!(parsed.value, "${MISSING} $$OTHER/lower/last");
}

#[test]
fn compose_variable_names_follow_unicode_regex_case_folding() {
    let parsed: Config = from_str_with_options(
        "value: '${K:-fallback}/$ſ/$SETK/$Ω'",
        compose_options(&[("K", "kelvin"), ("ſ", "long-s"), ("SETK", "greedy")]),
    )
    .unwrap();
    assert_eq!(parsed.value, "kelvin/long-s/greedy/$Ω");
    for syntax in [PropertySyntax::Braced, PropertySyntax::BracedOrBare] {
        let options =
            serde_saphyr::options! { property_syntax: syntax }.with_properties(HashMap::new());
        let literal: String = from_str_with_options("$K $ſ", options.clone()).unwrap();
        assert_eq!(literal, "$K $ſ");
        assert!(from_str_with_options::<String>("${K:-fallback}", options).is_err());
    }
}

#[rstest]
#[case::unclosed("${SET")]
#[case::unclosed_operator("${SET:-fallback")]
#[case::empty_name("${}")]
#[case::leading_space("${ SET}")]
#[case::trailing_space("${SET }")]
#[case::leading_digit("${1SET}")]
#[case::unicode_name("${NÄME}")]
#[case::unsupported_operator("${SET/fallback}")]
#[case::unsupported_colon_operator("${SET:default}")]
fn malformed_references_are_errors(#[case] template: &str) {
    let err = from_str_with_options::<Config>(
        &format!("value: '{template}'\n"),
        compose_options(&[("SET", "value")]),
    )
    .unwrap_err();
    assert_ne!(err.location(), Some(Location::UNKNOWN));
    assert!(err.location().is_some());
}

#[rstest]
#[case::selected("value: |-\n  ${MISSING:-first\n  second}\n")]
#[case::unused("value: |-\n  ${SET:-first\n  second}\n")]
fn multiline_operator_references_are_invalid(#[case] yaml: &str) {
    let err =
        from_str_with_options::<Config>(yaml, compose_options(&[("SET", "value")])).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        Error::InvalidPropertyName { .. }
    ));
}

#[test]
fn missing_direct_references_expand_to_empty() {
    let options = compose_options(&[("EMPTY", "")]);
    let parsed: HashMap<String, String> = from_str_with_options(
        "first: '$BARE ${BRACED}'\nsecond: ${EMPTY}\nthird: ${OTHER}\n",
        options,
    )
    .unwrap();
    assert_eq!(parsed["first"], " ");
    assert_eq!(parsed["second"], "");
    assert_eq!(parsed["third"], "");
}

#[test]
fn repeated_missing_references_and_aliases_expand_to_empty() {
    let options = compose_options(&[]);
    let parsed: HashMap<String, String> = from_str_with_options(
        "first: &missing '${MISSING}/$MISSING'\nsecond: *missing\nthird: $MISSING\n",
        options,
    )
    .unwrap();
    assert_eq!(parsed["first"], "/");
    assert_eq!(parsed["second"], "/");
    assert_eq!(parsed["third"], "");
}

#[test]
fn default_and_inactive_operator_branches_are_evaluated_lazily() {
    let options = compose_options(&[("SET", "value"), ("EMPTY", "")]);
    let parsed: Vec<String> = from_str_with_options(
        "- '${MISSING:-default}'\n\
         - '${SET:-$MISSING}'\n\
         - '${SET-${MISSING?failure}}'\n\
         - '${MISSING+$ALSO_MISSING}'\n\
         - '${EMPTY:+${MISSING?failure}}'\n\
         - '$$MISSING'\n",
        options,
    )
    .unwrap();
    assert_eq!(parsed, ["default", "value", "value", "", "", "$MISSING"]);
}

#[test]
fn compose_mode_without_property_map_uses_empty_map() {
    let options = serde_saphyr::options! {
        property_syntax: PropertySyntax::DockerCompose,
    };
    let parsed: Config =
        from_str_with_options("value: '${PATH:-fallback}/$PATH/$$PATH'\n", options).unwrap();
    assert_eq!(parsed.value, "fallback//$PATH");
}

#[test]
fn preserves_mapping_keys_and_interpolates_aliased_and_merged_values() {
    let yaml = "\
defaults: &defaults
  '${KEY}': &shared \"$VALUE\"
  $KEY: *shared
merged:
  <<: *defaults
  \"${KEY}suffix\": '$VALUE'
";
    let options = compose_options(&[("KEY", "renamed"), ("VALUE", "${FINAL} $$FINAL")]);
    for parsed in [
        from_str_with_options::<HashMap<String, HashMap<String, String>>>(yaml, options.clone()),
        from_reader_with_options(yaml.as_bytes(), options),
    ] {
        let parsed = parsed.unwrap();
        assert_eq!(parsed["defaults"]["${KEY}"], "${FINAL} $$FINAL");
        assert_eq!(parsed["defaults"]["$KEY"], "${FINAL} $$FINAL");
        assert_eq!(parsed["merged"]["${KEY}"], "${FINAL} $$FINAL");
        assert_eq!(parsed["merged"]["$KEY"], "${FINAL} $$FINAL");
        assert_eq!(parsed["merged"]["${KEY}suffix"], "${FINAL} $$FINAL");
        assert!(!parsed["defaults"].contains_key("renamed"));
    }
}

#[test]
fn compound_mapping_keys_preserve_their_string_contents() {
    let options = compose_options(&[("KEY", "renamed"), ("VALUE", "expanded")]);
    let sequence_key: HashMap<Vec<String>, String> =
        from_str_with_options("? ['$KEY', \"${KEY}\"]\n: '$VALUE'\n", options.clone()).unwrap();
    assert_eq!(
        sequence_key,
        HashMap::from([(
            vec!["$KEY".to_owned(), "${KEY}".to_owned()],
            "expanded".to_owned()
        )])
    );
    let mapping_key: std::collections::BTreeMap<
        std::collections::BTreeMap<String, String>,
        String,
    > = from_str_with_options("? {nested: '$KEY'}\n: '$VALUE'\n", options).unwrap();
    assert_eq!(
        mapping_key,
        std::collections::BTreeMap::from([(
            std::collections::BTreeMap::from([("nested".to_owned(), "$KEY".to_owned())]),
            "expanded".to_owned(),
        )])
    );
}

#[test]
fn binary_strings_and_bytes_are_decoded_without_interpolation() {
    let options = compose_options(&[("SET", "expanded")]);
    let value: String = from_str_with_options("!!binary JFNFVA==\n", options.clone()).unwrap();
    assert_eq!(value, "$SET");
    let bytes: serde_bytes::ByteBuf =
        from_str_with_options("!!binary JFNFVA==\n", options.clone()).unwrap();
    assert_eq!(bytes.as_ref(), b"$SET");
    let sequence: Vec<u8> = from_str_with_options("!!binary JFNFVA==\n", options.clone()).unwrap();
    assert_eq!(sequence, b"$SET");
    let generic: serde_json::Value =
        from_str_with_options("!!binary JFNFVA==\n", options.clone()).unwrap();
    assert_eq!(generic, "$SET");
    let non_utf8: serde_bytes::ByteBuf = from_str_with_options("!!binary /w==\n", options).unwrap();
    assert_eq!(non_utf8.as_ref(), &[0xff]);
}

#[test]
fn interpolated_yaml_strings_are_not_retyped() {
    let options = compose_options(&[
        ("NULL", "null"),
        ("BOOLEAN", "true"),
        ("NUMBER", "42"),
        ("EMPTY", ""),
    ]);
    let parsed: serde_json::Value = from_str_with_options(
        "expanded_null: ${NULL}\n\
         expanded_bool: $BOOLEAN\n\
         expanded_number: ${NUMBER}\n\
         expanded_empty: ${EMPTY}\n\
         missing: ${MISSING}\n\
         yaml_null: null\n\
         yaml_bool: true\n\
         yaml_number: 42\n",
        options,
    )
    .unwrap();
    assert_eq!(
        parsed,
        serde_json::json!({
            "expanded_null": "null", "expanded_bool": "true", "expanded_number": "42",
            "expanded_empty": "", "missing": "", "yaml_null": null,
            "yaml_bool": true, "yaml_number": 42,
        })
    );
}

#[test]
fn explicitly_requested_scalar_types_parse_expanded_strings() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Typed {
        port: u16,
        enabled: bool,
        marker: char,
        optional: Option<u32>,
    }
    let parsed: Typed = from_str_with_options(
        "port: '$PORT'\nenabled: $ENABLED\nmarker: ${MARKER}\noptional: \"${PORT}\"\n",
        compose_options(&[("PORT", "8080"), ("ENABLED", "true"), ("MARKER", "~")]),
    )
    .unwrap();
    assert_eq!(
        parsed,
        Typed {
            port: 8080,
            enabled: true,
            marker: '~',
            optional: Some(8080)
        }
    );
}

#[test]
fn explicit_nonstring_tags_are_not_interpolated() {
    let options = compose_options(&[("PORT", "8080")]);
    assert!(from_str_with_options::<u16>("!!int '${PORT}'", options.clone()).is_err());
    assert!(from_str_with_options::<bool>("!!bool '${MISSING}'", options).is_err());
}

#[rstest]
#[case::empty("${EMPTY}", "")]
#[case::null("${NULL}", "null")]
#[case::null_default("${MISSING:-null}", "null")]
#[case::empty_default("${MISSING:-}", "")]
#[case::tilde_default("${MISSING:-~}", "~")]
#[case::missing("${MISSING}", "")]
fn interpolated_nullish_strings_remain_some(#[case] template: &str, #[case] expected: &str) {
    let parsed: Option<String> = from_str_with_options(
        template,
        compose_options(&[("EMPTY", ""), ("NULL", "null")]),
    )
    .unwrap();
    assert_eq!(parsed.as_deref(), Some(expected));
}

#[test]
fn interpolated_strings_select_the_string_variant_of_untagged_enums() {
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(untagged)]
    enum Value {
        Boolean(bool),
        Number(u64),
        Text(String),
    }
    let parsed: Vec<Value> = from_str_with_options(
        "- ${NUMBER}\n- $BOOLEAN\n- '${NULL}'\n- |-\n  ${EMPTY}\n",
        compose_options(&[
            ("NUMBER", "42"),
            ("BOOLEAN", "true"),
            ("NULL", "null"),
            ("EMPTY", ""),
        ]),
    )
    .unwrap();
    assert_eq!(
        parsed,
        ["42", "true", "null", ""].map(|text| Value::Text(text.to_owned()))
    );
}

#[test]
fn expanded_enum_names_remain_strings_in_no_schema_mode() {
    #[derive(Debug, Deserialize, PartialEq)]
    enum Choice {
        #[serde(rename = "true")]
        BooleanName,
        #[serde(rename = "123")]
        NumericName,
    }
    for (name, expected) in [("true", Choice::BooleanName), ("123", Choice::NumericName)] {
        let mut options = compose_options(&[("CHOICE", name)]);
        options.no_schema = true;
        let parsed: Choice = from_str_with_options("${CHOICE}", options).unwrap();
        assert_eq!(parsed, expected);
    }
}

#[derive(Debug)]
struct RejectedString;

impl<'de> Deserialize<'de> for RejectedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Err(serde::de::Error::custom(format!(
            "rejected secret: {value}"
        )))
    }
}

#[rstest]
#[case::single_quoted("'$SECRET'\n")]
#[case::double_quoted("\"${SECRET}\"\n")]
#[case::literal("|-\n  ${SECRET}\n")]
#[case::folded(">-\n  $SECRET\n")]
fn custom_errors_redact_interpolated_quoted_and_block_strings(#[case] yaml: &str) {
    let options = compose_options(&[("SECRET", "sensitive-property-value")]);
    for err in [
        from_str_with_options::<RejectedString>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, RejectedString>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        let message = err.to_string();
        assert!(!message.contains("sensitive-property-value"), "{message}");
        assert!(
            message.contains("SECRET") || message.contains("interpolated"),
            "{message}"
        );
    }
}

#[test]
fn nested_expansion_respects_depth_budget() {
    let options = serde_saphyr::options! {
        property_syntax: PropertySyntax::DockerCompose,
        budget: serde_saphyr::budget! { max_property_expansion_depth: 2 },
    };
    let yaml = "value: '${MISSING:-${MISSING:-${MISSING:-${MISSING:-value}}}}'\n";
    for err in [
        from_str_with_options::<Config>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, Config>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        assert!(matches!(
            err.without_snippet(),
            Error::Budget {
                breach: BudgetBreach::PropertyExpansionDepth {
                    depth: 3,
                    max_depth: 2
                },
                ..
            }
        ));
    }
}

#[rstest]
#[case::default("${MISSING:-cost $100}", 0, "cost $100")]
#[case::replacement("${SET:+cost $100}", 0, "cost $100")]
#[case::other_literal_dollars("${MISSING:-$! $Ω $}", 0, "$! $Ω $")]
#[case::nested_default("${MISSING:-${MISSING:-cost $100}}", 1, "cost $100")]
fn literal_dollars_do_not_consume_expansion_depth(
    #[case] template: &str,
    #[case] max_depth: usize,
    #[case] expected: &str,
) {
    let mut options = compose_options(&[("SET", "value")]);
    options.budget = serde_saphyr::budget! {
        max_property_expansion_depth: max_depth,
    };
    let yaml = format!("value: '{template}'\n");
    for parsed in [
        from_str_with_options::<Config>(&yaml, options.clone()).unwrap(),
        from_reader_with_options::<_, Config>(yaml.as_bytes(), options).unwrap(),
    ] {
        assert_eq!(parsed.value, expected);
    }
}

#[rstest]
#[case::unset("${MISSING?cost $100}", false)]
#[case::empty("${EMPTY:?cost $100}", true)]
fn literal_dollar_required_hints_preserve_required_errors(
    #[case] template: &str,
    #[case] empty: bool,
) {
    let mut options = compose_options(&[("EMPTY", "")]);
    options.budget = serde_saphyr::budget! { max_property_expansion_depth: 0 };
    let yaml = format!("value: '{template}'\n");
    for err in [
        from_str_with_options::<Config>(&yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, Config>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        match err.without_snippet() {
            Error::PropertyRequiredButEmpty { name, message, .. } if empty => {
                assert_eq!(name, "EMPTY");
                assert_eq!(message, "cost $100");
            }
            Error::PropertyRequiredButUnset { name, message, .. } if !empty => {
                assert_eq!(name, "MISSING");
                assert_eq!(message, "cost $100");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

#[rstest]
#[case::bare("${MISSING:-$SET}")]
#[case::braced("${MISSING:-${SET}}")]
#[case::escape("${MISSING:-$$SET}")]
#[case::kelvin("${MISSING:-$K}")]
#[case::long_s("${MISSING:-$ſ}")]
#[case::after_literal_dollar("${MISSING:-$100 $SET}")]
fn references_and_escapes_in_operator_text_consume_expansion_depth(#[case] template: &str) {
    let mut options = compose_options(&[("SET", "value")]);
    options.budget = serde_saphyr::budget! { max_property_expansion_depth: 0 };
    let yaml = format!("value: '{template}'\n");
    for err in [
        from_str_with_options::<Config>(&yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, Config>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        assert!(matches!(
            err.without_snippet(),
            Error::Budget {
                breach: BudgetBreach::PropertyExpansionDepth {
                    depth: 1,
                    max_depth: 0
                },
                ..
            }
        ));
    }
}

#[rstest]
#[case::bare("$SET")]
#[case::single_quoted("'$SET'")]
#[case::double_quoted("\"${SET}\"")]
#[case::block("|-\n  ${SET}\n")]
#[case::escape("'$$SET'")]
fn interpolation_respects_work_budget_in_every_style(#[case] yaml: &str) {
    let options = serde_saphyr::options! {
        property_syntax: PropertySyntax::DockerCompose,
        budget: serde_saphyr::budget! { max_total_property_interpolation_work: 0 },
    }
    .with_properties(HashMap::from([("SET".to_owned(), "value".to_owned())]));
    let err = from_str_with_options::<String>(yaml, options).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::PropertyInterpolationWork { .. },
            ..
        }
    ));
}
