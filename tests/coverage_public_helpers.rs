#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::value::{MapDeserializer, U64Deserializer, UnitDeserializer};
use serde_saphyr::{
    Error, ExternalMessage, ExternalMessageSource, IncludeRequest, Location, Span, Spanned, budget,
};

#[test]
fn standalone_budget_helper_reports_accept_reject_and_scan_error() {
    assert!(!budget::parse_yaml("answer: 42\n", budget::Budget::default()).unwrap());

    let mut reject_every_event = budget::Budget::default();
    reject_every_event.max_events = 0;
    assert!(budget::parse_yaml("answer: 42\n", reject_every_event).unwrap());

    assert!(budget::parse_yaml("[", budget::Budget::default()).is_err());
}

#[cfg(feature = "serde_derived_types")]
#[test]
fn budget_deserialization_defaults_the_new_comment_limit() {
    let default = budget::Budget::default();
    let mut json = serde_json::to_value(&default).unwrap();
    json.as_object_mut()
        .unwrap()
        .remove("max_total_comment_bytes");

    let restored: budget::Budget = serde_json::from_value(json).unwrap();
    assert_eq!(
        restored.max_total_comment_bytes,
        default.max_total_comment_bytes
    );
}

#[test]
fn public_message_and_include_builders_keep_attached_metadata() {
    let params = [("minimum".to_owned(), "3".to_owned())];
    let message = ExternalMessage::new(ExternalMessageSource::Validator, "too short")
        .with_code("length")
        .with_params(&params);

    assert_eq!(message.code, Some("length"));
    assert_eq!(message.params, params);

    let request = IncludeRequest::new("child.yaml", "root.yaml", Location::UNKNOWN)
        .with_from_id("/config/root.yaml")
        .with_stack(vec!["root.yaml".to_owned(), "child.yaml".to_owned()])
        .with_size_remaining(128);

    assert_eq!(request.from_id, Some("/config/root.yaml"));
    assert_eq!(request.stack, ["root.yaml", "child.yaml"]);
    assert_eq!(request.size_remaining, Some(128));
}

#[test]
fn location_and_span_reject_invalid_deserializer_shapes() {
    type NumericDeserializer = U64Deserializer<Error>;

    let location_map = [(NumericDeserializer::new(1), NumericDeserializer::new(2))];
    let location_error =
        Location::deserialize(MapDeserializer::<_, Error>::new(location_map.into_iter()))
            .unwrap_err();
    assert!(matches!(location_error, Error::SerdeInvalidType { .. }));

    let span_map = [(NumericDeserializer::new(1), NumericDeserializer::new(2))];
    let span_error =
        Span::deserialize(MapDeserializer::<_, Error>::new(span_map.into_iter())).unwrap_err();
    assert!(matches!(span_error, Error::SerdeInvalidType { .. }));

    let location_shape_error = Location::deserialize(UnitDeserializer::<Error>::new()).unwrap_err();
    assert!(matches!(
        location_shape_error,
        Error::SerdeInvalidType { .. }
    ));

    let span_shape_error = Span::deserialize(UnitDeserializer::<Error>::new()).unwrap_err();
    assert!(matches!(span_shape_error, Error::SerdeInvalidType { .. }));
}

#[cfg(feature = "huge_documents")]
#[test]
fn span_index_deserialization_saturates_values_above_48_bits() {
    let span: Span = serde_json::from_value(serde_json::json!({
        "offset": u64::MAX,
        "len": u64::MAX,
        "byte_info": [u64::MAX, u64::MAX]
    }))
    .unwrap();

    let packed_max = (1_u64 << 48) - 1;
    assert_eq!(span.offset(), packed_max);
    assert_eq!(span.len(), packed_max);
    assert_eq!(span.byte_offset(), None);
    assert_eq!(span.byte_len(), None);
}

#[test]
fn spanned_empty_map_uses_unknown_location_fallback() {
    let empty: Spanned<std::collections::BTreeMap<String, u32>> =
        serde_json::from_str("{}").unwrap();
    assert!(empty.value.is_empty());
    assert_eq!(empty.referenced, Location::UNKNOWN);
    assert_eq!(empty.defined, Location::UNKNOWN);
}

#[cfg(feature = "include")]
#[test]
fn include_tags_reject_sequence_nodes() {
    use serde::de::IgnoredAny;
    use serde_saphyr::{Error, IncludeResolveError, ResolvedInclude};

    let options = serde_saphyr::options! {}.with_include_resolver(
        |_request: IncludeRequest<'_>| -> Result<ResolvedInclude, IncludeResolveError> {
            panic!("invalid include forms must be rejected before resolution")
        },
    );
    let sequence_error =
        serde_saphyr::from_str_with_options::<IgnoredAny>("!include [child.yaml]\n", options)
            .unwrap_err();
    assert!(matches!(
        sequence_error.without_snippet(),
        Error::UnsupportedIncludeForm { .. }
    ));
}

#[cfg(feature = "robotics")]
#[test]
fn robotics_number_parser_reports_deep_and_malformed_sexagesimal_inputs() {
    fn parse_error(input: &str) -> Error {
        let options = serde_saphyr::options! { angle_conversions: true };
        serde_saphyr::from_str_with_options::<f64>(input, options).unwrap_err()
    }

    for input in ["deg(1:2:60)", "deg(1:4294967296)", "deg(1:2:3.)"] {
        let error = parse_error(input);
        assert!(matches!(error.without_snippet(), Error::HookError { .. }));
    }

    let deeply_nested = format!("{}1{}", "(".repeat(257), ")".repeat(257));
    let error = parse_error(&deeply_nested);
    assert!(matches!(error.without_snippet(), Error::HookError { .. }));
}

#[test]
fn scalar_number_edges_cover_legacy_zero_and_signed_underscore_validation() {
    let legacy = serde_saphyr::options! { legacy_octal_numbers: true };
    assert_eq!(
        serde_saphyr::from_str_with_options::<u64>("0", legacy).unwrap(),
        0
    );

    assert!(serde_saphyr::from_str::<u64>("1__0").is_err());
    assert!(serde_saphyr::from_str::<i64>("-1__0").is_err());

    let negative_zero: serde_json::Value = serde_saphyr::from_str("-0").unwrap();
    assert_eq!(negative_zero, serde_json::json!(0));
}
