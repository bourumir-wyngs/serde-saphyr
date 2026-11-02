#![cfg(feature = "properties")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{
    DuplicateKeyPolicy, Error, Options, PropertySyntax, budget::BudgetBreach,
    from_multiple_with_options, from_slice_multiple_with_options, from_str_multiple_with_options,
    from_str_with_options, read_with_options,
};
use std::collections::BTreeMap;

const VARS: &[(&str, &str)] = &[
    ("SET", "value"),
    ("EMPTY", ""),
    ("NUL", "null"),
    ("TILDE", "~"),
    ("NUL_CAPS", "NULL"),
    ("PORT", "5432"),
    ("FLAG", "true"),
];

fn options(syntax: PropertySyntax) -> Options {
    serde_saphyr::options! {
        property_syntax: syntax,
    }
    .with_properties(
        VARS.iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
    )
}

fn with_nulls() -> Options {
    options(PropertySyntax::DockerComposeWithNulls)
}

#[derive(Debug, Deserialize, PartialEq)]
struct Config<T> {
    value: T,
}

fn load<T: for<'de> Deserialize<'de>>(yaml: &str, options: Options) -> Result<T, String> {
    from_str_with_options::<Config<T>>(yaml, options)
        .map(|config| config.value)
        .map_err(|err| err.to_string())
}

#[rstest]
// A value that is solely a reference to an unset property is null, whatever its style.
#[case::bare_unset("value: $MISSING\n", None)]
#[case::braced_unset("value: ${MISSING}\n", None)]
#[case::single_quoted_unset("value: '$MISSING'\n", None)]
#[case::double_quoted_unset("value: \"${MISSING}\"\n", None)]
#[case::stripped_block_unset("value: |-\n  $MISSING\n", None)]
#[case::folded_stripped_unset("value: >-\n  $MISSING\n", None)]
// A clipped block keeps its newline, so it is not a sole reference.
#[case::clipped_block_unset("value: |\n  $MISSING\n", Some("\n"))]
// An explicitly empty property is an empty string.
#[case::bare_empty("value: $EMPTY\n", Some(""))]
#[case::braced_empty("value: ${EMPTY}\n", Some(""))]
#[case::quoted_empty("value: '$EMPTY'\n", Some(""))]
// Expanded null markers are null.
#[case::expanded_null("value: $NUL\n", None)]
#[case::expanded_tilde("value: ${TILDE}\n", None)]
#[case::expanded_null_caps("value: $NUL_CAPS\n", None)]
#[case::quoted_expanded_null("value: \"$NUL\"\n", None)]
#[case::default_null("value: ${MISSING:-null}\n", None)]
#[case::default_tilde("value: ${MISSING-~}\n", None)]
// Selected empty defaults stay empty strings.
#[case::nested_unset_default("value: ${A:-${B}}\n", Some(""))]
#[case::empty_default("value: ${MISSING-}\n", Some(""))]
#[case::empty_nonempty_default("value: ${MISSING:-}\n", Some(""))]
#[case::empty_alternative("value: ${MISSING+x}\n", Some(""))]
// Surrounding text is kept.
#[case::prefix_unset("value: prefix-$MISSING\n", Some("prefix-"))]
#[case::suffix_unset("value: ${MISSING}-suffix\n", Some("-suffix"))]
#[case::prefix_null("value: prefix-$NUL\n", Some("prefix-null"))]
#[case::two_unset("value: $MISSING$MISSING\n", Some(""))]
#[case::escaped("value: $$MISSING\n", Some("$MISSING"))]
#[case::set("value: $SET\n", Some("value"))]
// String-forcing tags keep their meaning.
#[case::str_tag_unset("value: !!str $MISSING\n", Some(""))]
#[case::non_specific_tag_unset("value: ! $MISSING\n", Some(""))]
#[case::str_tag_null("value: !!str $NUL\n", Some("null"))]
// Scalars without an expansion resolve as in every other mode.
#[case::literal_null("value: null\n", None)]
#[case::literal_tilde("value: ~\n", None)]
#[case::literal_empty("value:\n", None)]
#[case::quoted_literal_null("value: 'null'\n", Some("null"))]
#[case::quoted_literal_empty("value: ''\n", Some(""))]
fn option_string(#[case] yaml: &str, #[case] expected: Option<&str>) {
    let value: Option<String> = load(yaml, with_nulls()).unwrap();
    assert_eq!(value.as_deref(), expected, "{yaml}");
}

#[rstest]
#[case::unset("value: $MISSING\n", serde_json::Value::Null)]
#[case::expanded_null("value: $NUL\n", serde_json::Value::Null)]
#[case::empty("value: $EMPTY\n", serde_json::json!(""))]
// Expanded numbers and booleans stay strings for generic targets, as in `DockerCompose`.
#[case::number("value: $PORT\n", serde_json::json!("5432"))]
#[case::boolean("value: $FLAG\n", serde_json::json!("true"))]
#[case::literal_number("value: 5432\n", serde_json::json!(5432))]
fn untyped(#[case] yaml: &str, #[case] expected: serde_json::Value) {
    let value: serde_json::Value = load(yaml, with_nulls()).unwrap();
    assert_eq!(value, expected, "{yaml}");
}

#[test]
fn typed_conversions_are_unchanged() {
    assert_eq!(load::<u16>("value: $PORT\n", with_nulls()), Ok(5432));
    assert_eq!(
        load::<Option<u16>>("value: $PORT\n", with_nulls()),
        Ok(Some(5432))
    );
    assert_eq!(
        load::<Option<u16>>("value: $MISSING\n", with_nulls()),
        Ok(None)
    );
    assert_eq!(load::<bool>("value: \"$FLAG\"\n", with_nulls()), Ok(true));
    assert_eq!(
        load::<Option<bool>>("value: $MISSING\n", with_nulls()),
        Ok(None)
    );
    assert_eq!(
        load::<Option<bool>>("value: $NUL\n", with_nulls()),
        Ok(None)
    );
}

/// A value that becomes null behaves exactly like a literal null in the same position.
#[rstest]
#[case::unset("value: $MISSING\n")]
#[case::expanded_null("value: $NUL\n")]
fn null_into_non_option_matches_literal_null(#[case] yaml: &str) {
    let literal = load::<String>("value: null\n", with_nulls());
    let expanded = load::<String>(yaml, with_nulls());
    assert_eq!(
        expanded.is_ok(),
        literal.is_ok(),
        "{expanded:?} vs {literal:?}"
    );
    assert!(load::<bool>(yaml, with_nulls()).is_err());
    assert!(load::<u16>(yaml, with_nulls()).is_err());
}

#[rstest]
#[case::unset_or_empty_unset("value: ${MISSING:?must be set}\n")]
#[case::unset_or_empty_empty("value: ${EMPTY:?must be set}\n")]
#[case::unset_only("value: ${MISSING?must be set}\n")]
fn required_references_still_error(#[case] yaml: &str) {
    let err = load::<Option<String>>(yaml, with_nulls()).unwrap_err();
    assert!(err.contains("must be set"), "{err}");
    assert!(err.contains("line 1 column 8"), "{err}");
}

#[test]
fn aliases_resolve_to_the_same_null() {
    let parsed: BTreeMap<String, Option<String>> =
        from_str_with_options("a: &x $MISSING\nb: *x\nc: &y $EMPTY\nd: *y\n", with_nulls())
            .unwrap();
    assert_eq!(parsed["a"], None);
    assert_eq!(parsed["b"], None);
    assert_eq!(parsed["c"].as_deref(), Some(""));
    assert_eq!(parsed["d"].as_deref(), Some(""));
}

#[test]
fn keys_are_not_interpolated() {
    let parsed: BTreeMap<String, Option<String>> =
        from_str_with_options("$MISSING: $MISSING\n$NUL: x\n", with_nulls()).unwrap();
    assert_eq!(parsed.get("$MISSING"), Some(&None));
    assert_eq!(parsed.get("$NUL").cloned().flatten().as_deref(), Some("x"));
}

#[test]
fn values_in_nested_collections_become_null() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Nested {
        list: Vec<Option<String>>,
        map: BTreeMap<String, Option<u16>>,
    }
    let parsed: Nested = from_str_with_options(
        "list: [$MISSING, $EMPTY, $SET]\nmap:\n  port: $PORT\n  other: ${MISSING}\n",
        with_nulls(),
    )
    .unwrap();
    assert_eq!(
        parsed.list,
        [None, Some(String::new()), Some("value".into())]
    );
    assert_eq!(parsed.map["port"], Some(5432));
    assert_eq!(parsed.map["other"], None);
}

/// The plain Compose mode keeps its string-preserving behavior.
#[rstest]
#[case::unset("value: $MISSING\n", Some(""))]
#[case::expanded_null("value: $NUL\n", Some("null"))]
#[case::expanded_tilde("value: $TILDE\n", Some("~"))]
#[case::empty("value: $EMPTY\n", Some(""))]
fn docker_compose_is_unchanged(#[case] yaml: &str, #[case] expected: Option<&str>) {
    let value: Option<String> = load(yaml, options(PropertySyntax::DockerCompose)).unwrap();
    assert_eq!(value.as_deref(), expected, "{yaml}");
}

#[test]
fn existing_variants_keep_their_integer_values() {
    assert_eq!(PropertySyntax::Braced as u8, 0);
    assert_eq!(PropertySyntax::BracedOrBare as u8, 1);
    assert_eq!(PropertySyntax::DockerCompose as u8, 2);
    assert_eq!(PropertySyntax::DockerComposeWithNulls as u8, 3);
}

#[test]
fn works_without_a_property_map() {
    let options = serde_saphyr::options! {
        property_syntax: PropertySyntax::DockerComposeWithNulls,
    };
    assert_eq!(
        load::<Option<String>>("value: $MISSING\n", options.clone()),
        Ok(None)
    );
    assert_eq!(
        load::<Option<String>>("value: a-$MISSING\n", options),
        Ok(Some("a-".to_owned()))
    );
}

fn document_results<T: for<'de> Deserialize<'de>>(
    yaml: &str,
    options: Options,
) -> [Result<Vec<T>, Error>; 4] {
    let mut reader = yaml.as_bytes();
    [
        from_multiple_with_options(yaml, options.clone()),
        from_str_multiple_with_options(yaml, options.clone()),
        from_slice_multiple_with_options(yaml.as_bytes(), options.clone()),
        read_with_options(&mut reader, options).collect(),
    ]
}

fn document_values<T: for<'de> Deserialize<'de>>(yaml: &str, options: Options) -> [Vec<T>; 4] {
    document_results(yaml, options).map(Result::unwrap)
}

#[rstest]
#[case::literal("null")]
#[case::missing("$MISSING")]
#[case::marker("$NUL")]
#[case::quoted("'${MISSING}'")]
#[case::default("${MISSING:-null}")]
fn null_documents_are_skipped(#[case] scalar: &str) {
    let yaml = format!("{scalar}\n---\nkept\n");
    for values in document_values::<String>(&yaml, with_nulls()) {
        assert_eq!(values, ["kept"]);
    }
    for values in document_values::<Option<String>>(&yaml, with_nulls()) {
        assert_eq!(values, [Some("kept".to_owned())]);
    }
}

#[rstest]
#[case::empty("$EMPTY", "")]
#[case::string_tag("!!str $MISSING", "")]
#[case::non_specific_tag("! $MISSING", "")]
#[case::quoted_literal("'null'", "null")]
fn non_null_documents_are_preserved(#[case] scalar: &str, #[case] expected: &str) {
    let yaml = format!("{scalar}\n---\nkept\n");
    for values in document_values::<String>(&yaml, with_nulls()) {
        assert_eq!(values, [expected, "kept"]);
    }
}

#[test]
fn docker_compose_keeps_expanded_null_documents_as_strings() {
    for values in document_values::<String>(
        "$MISSING\n---\n$NUL\n",
        options(PropertySyntax::DockerCompose),
    ) {
        assert_eq!(values, ["", "null"]);
    }
}

#[rstest]
#[case::literal("value:\n  <<: null\n  name: kept\n")]
#[case::live("value:\n  <<: $MISSING\n  name: kept\n")]
#[case::marker("value:\n  <<: $NUL\n  name: kept\n")]
#[case::replay("value:\n  <<: [$MISSING, {name: kept}]\n")]
#[case::alias("anchor: &x $MISSING\nvalue:\n  <<: *x\n  name: kept\n")]
fn null_merge_values_are_no_ops(
    #[case] yaml: &str,
    #[values(DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins)] policy: DuplicateKeyPolicy,
) {
    let mut options = with_nulls();
    options.duplicate_keys = policy;
    let value: BTreeMap<String, String> = load(yaml, options).unwrap();
    assert_eq!(value, BTreeMap::from([("name".into(), "kept".into())]));
}

#[derive(Debug, Deserialize, PartialEq)]
enum NullPayload {
    Unit,
    Maybe(Option<String>),
    Text(String),
    Number(u16),
}

#[rstest]
#[case::mapping_literal("Unit: null", NullPayload::Unit)]
#[case::mapping_missing("Unit: $MISSING", NullPayload::Unit)]
#[case::mapping_marker("Unit: $NUL", NullPayload::Unit)]
#[case::tagged_literal("!Unit null", NullPayload::Unit)]
#[case::tagged_missing("!Unit $MISSING", NullPayload::Unit)]
#[case::newtype_literal("!Maybe null", NullPayload::Maybe(None))]
#[case::newtype_missing("!Maybe $MISSING", NullPayload::Maybe(None))]
#[case::newtype_default("!Maybe ${MISSING:-null}", NullPayload::Maybe(None))]
fn enum_payloads_accept_null(#[case] yaml: &str, #[case] expected: NullPayload) {
    let value: NullPayload = from_str_with_options(yaml, with_nulls()).unwrap();
    assert_eq!(value, expected);
}

#[rstest]
#[case::string_payload("!Text $MISSING")]
#[case::standard_tag("!!null $MISSING")]
#[case::unmatched_tag("!Unknown $MISSING")]
fn enum_tags_and_payload_types_remain_checked(#[case] yaml: &str) {
    assert!(from_str_with_options::<NullPayload>(yaml, with_nulls()).is_err());
}

#[rstest]
fn string_forcing_tags_are_not_null_merge_or_unit_payloads(#[values("!!str", "!")] tag: &str) {
    let merge = format!("value:\n  <<: {tag} $MISSING\n");
    assert!(load::<BTreeMap<String, String>>(&merge, with_nulls()).is_err());
    let unit = format!("Unit: {tag} $MISSING\n");
    assert!(from_str_with_options::<NullPayload>(&unit, with_nulls()).is_err());
}

#[test]
fn enum_dispatch_preserves_compose_strings_and_checks_payload_schema() {
    let value: NullPayload =
        from_str_with_options("!Maybe $MISSING", options(PropertySyntax::DockerCompose)).unwrap();
    assert_eq!(value, NullPayload::Maybe(Some(String::new())));

    let mut options = with_nulls();
    options.no_schema = true;
    let value: NullPayload = from_str_with_options("!Number 42", options.clone()).unwrap();
    assert_eq!(value, NullPayload::Number(42));
    assert!(from_str_with_options::<NullPayload>("!Text 42", options).is_err());
}

#[test]
fn null_document_probe_preserves_required_errors_and_reader_recovery() {
    let yaml = "${MISSING:?required}\n---\nkept\n";
    let error = from_multiple_with_options::<String>(yaml, with_nulls()).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::PropertyRequiredButUnset { .. }
    ));
    assert!(error.to_string().contains("line 1 column 1"), "{error}");

    let mut reader = yaml.as_bytes();
    let mut values = read_with_options::<_, String>(&mut reader, with_nulls());
    let error = values.next().unwrap().unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::PropertyRequiredButUnset { .. }
    ));
    assert_eq!(values.next().unwrap().unwrap(), "kept");
    assert!(values.next().is_none());
}

#[rstest]
#[case::missing("$MISSING")]
#[case::required("${MISSING:?required}")]
fn null_document_probe_enforces_interpolation_work_budget(#[case] first_document: &str) {
    let mut options = with_nulls();
    options.budget = serde_saphyr::budget! { max_total_property_interpolation_work: 0 };
    let yaml = format!("{first_document}\n---\nkept\n");
    let error = from_multiple_with_options::<String>(&yaml, options.clone()).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::PropertyInterpolationWork { .. },
            ..
        }
    ));

    let mut reader = yaml.as_bytes();
    let mut values = read_with_options::<_, String>(&mut reader, options);
    let error = values.next().unwrap().unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::PropertyInterpolationWork { .. },
            ..
        }
    ));
    assert!(values.next().is_none());
}

#[rstest]
#[case::literal("!Maybe null", NullPayload::Maybe(None))]
#[case::missing("!Maybe $MISSING", NullPayload::Maybe(None))]
#[case::marker("!Maybe $NUL", NullPayload::Maybe(None))]
#[case::default("!Maybe ${MISSING:-null}", NullPayload::Maybe(None))]
#[case::quoted("!Maybe '$MISSING'", NullPayload::Maybe(None))]
#[case::unit("!Unit $MISSING", NullPayload::Unit)]
fn tagged_documents_preserve_null_payloads(#[case] yaml: &str, #[case] expected: NullPayload) {
    let single: NullPayload = from_str_with_options(yaml, with_nulls()).unwrap();
    assert_eq!(single, expected);
    let stream = format!("{yaml}\n---\n!Number 42\n");
    for values in document_values::<NullPayload>(&stream, with_nulls()) {
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], expected);
        assert_eq!(values[1], NullPayload::Number(42));
    }
}

#[test]
fn tagged_documents_do_not_hide_invalid_null_payloads() {
    for result in document_results::<NullPayload>("!Text $MISSING", with_nulls()) {
        assert!(matches!(
            result.unwrap_err().without_snippet(),
            Error::NullIntoString { .. }
        ));
    }
}

fn with_work_budget(max_work: usize) -> Options {
    let mut options = with_nulls();
    options.budget = serde_saphyr::budget! {
        max_total_property_interpolation_work: max_work,
    };
    options
}

#[rstest]
#[case::property("$SET", "value")]
#[case::literal("literal", "literal")]
#[case::escaped_dollar("$$SET", "$SET")]
fn null_probe_work_budget_matches_single_document(#[case] yaml: &str, #[case] expected: &str) {
    // Find the single-document boundary rather than coupling the test to the number
    // of internal scalar peeks. Multi-document probing must not add charged work.
    let required_work = (0..=yaml.len() * 8 + 8)
        .find(|&max_work| from_str_with_options::<String>(yaml, with_work_budget(max_work)).is_ok())
        .expect("single scalar must fit the bounded test budget");
    assert!(required_work > 0);
    let value: String = from_str_with_options(yaml, with_work_budget(required_work)).unwrap();
    assert_eq!(value, expected);
    for values in document_values::<String>(yaml, with_work_budget(required_work)) {
        assert_eq!(values, [expected]);
    }

    let error =
        from_str_with_options::<String>(yaml, with_work_budget(required_work - 1)).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::PropertyInterpolationWork { .. },
            ..
        }
    ));
    for result in document_results::<String>(yaml, with_work_budget(required_work - 1)) {
        assert!(matches!(
            result.unwrap_err().without_snippet(),
            Error::Budget {
                breach: BudgetBreach::PropertyInterpolationWork { .. },
                ..
            }
        ));
    }
}

#[test]
fn skipped_null_documents_share_the_work_budget() {
    // Each bare missing reference costs ten byte inspections. Skipping the first
    // document must neither reset the counter nor expand the second one twice.
    let yaml = "$MISSING\n---\n$MISSING\n";
    for values in document_values::<String>(yaml, with_work_budget(20)) {
        assert!(values.is_empty());
    }
    for result in document_results::<String>(yaml, with_work_budget(19)) {
        assert!(matches!(
            result.unwrap_err().without_snippet(),
            Error::Budget {
                breach: BudgetBreach::PropertyInterpolationWork { .. },
                ..
            }
        ));
    }
}

#[test]
fn null_document_probe_redacts_required_property_hints() {
    let secret = "sentinel-secret-never-in-errors";
    let options = with_nulls().with_properties(std::collections::HashMap::from([(
        "SECRET".to_owned(),
        secret.to_owned(),
    )]));
    for result in document_results::<String>("${MISSING:?${SECRET}}", options) {
        let error = result.unwrap_err();
        assert!(matches!(
            error.without_snippet(),
            Error::PropertyRequiredButUnset { .. }
        ));
        assert!(!error.to_string().contains(secret), "{error}");
    }
}
