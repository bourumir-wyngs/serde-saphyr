#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_saphyr::{
    FlowMap, FlowSeq, NullableTilde, Tagged, from_str, from_str_with_options, options, to_string,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum Event {
    Timestamp,
    Count(u32),
    Maybe(Option<u32>),
    EmptyTuple(),
    EmptyStruct {},
}

fn timestamp() -> Tagged<Event> {
    Tagged(Event::Timestamp, Some("!timestamp".into()))
}

#[test]
fn unit_tag_round_trips_through_optional_wrappers() {
    let value = timestamp();
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!timestamp\n");
    assert_eq!(from_str::<Event>(&yaml).unwrap(), Event::Timestamp);
    assert_eq!(from_str::<Tagged<Event>>(&yaml).unwrap(), value);
    assert_eq!(
        from_str::<Option<Event>>(&yaml).unwrap(),
        Some(Event::Timestamp)
    );
    assert_eq!(
        from_str::<Option<Option<Event>>>(&yaml).unwrap(),
        Some(Some(Event::Timestamp)),
    );
    assert_eq!(to_string(&Some(timestamp())).unwrap(), yaml);
    assert_eq!(
        from_str::<Option<Tagged<Event>>>(&yaml).unwrap(),
        Some(timestamp()),
    );
    let optional = Tagged(Some(Event::Timestamp), Some("!timestamp".into()));
    assert_eq!(to_string(&optional).unwrap(), yaml);
    assert_eq!(from_str::<Tagged<Option<Event>>>(&yaml).unwrap(), optional);
    let nullable = Tagged(
        NullableTilde(Some(Event::Timestamp)),
        Some("!timestamp".into()),
    );
    assert_eq!(to_string(&nullable).unwrap(), yaml);
    assert_eq!(
        from_str::<Tagged<NullableTilde<Event>>>(&yaml).unwrap(),
        nullable
    );
}

#[rstest]
#[case("")]
#[case(" null")]
#[case(" Null")]
#[case(" NULL")]
#[case(" ~")]
fn unit_tag_accepts_omitted_or_explicit_null_payloads(
    #[case] payload: &str,
    #[values(false, true)] no_schema: bool,
) {
    let yaml = format!("!timestamp{payload}\n");
    let opts = options! { no_schema: no_schema };
    let direct: Event = from_str_with_options(&yaml, opts.clone()).unwrap();
    let optional: Option<Event> = from_str_with_options(&yaml, opts).unwrap();
    assert_eq!(direct, Event::Timestamp);
    assert_eq!(optional, Some(direct));
    let tagged: Tagged<Event> = from_str(&yaml).unwrap();
    assert_eq!(to_string(&tagged).unwrap(), "!timestamp\n");
}

#[rstest]
#[case("42")]
#[case("text")]
#[case("'null'")]
#[case("''")]
#[case("[]")]
#[case("{}")]
fn unit_tag_rejects_non_null_payloads(#[case] payload: &str) {
    let yaml = format!("!timestamp {payload}");
    assert!(from_str::<Event>(&yaml).is_err());
    assert!(from_str::<Option<Event>>(&yaml).is_err());
}

#[test]
fn tag_only_units_round_trip_in_block_and_flow_containers() {
    let sequence = vec![Some(timestamp()), None, Some(timestamp())];
    let block = to_string(&sequence).unwrap();
    assert_eq!(block, "- !timestamp\n- null\n- !timestamp\n");
    assert_eq!(
        from_str::<Vec<Option<Tagged<Event>>>>(&block).unwrap(),
        sequence
    );
    let flow = to_string(&FlowSeq(&sequence)).unwrap();
    assert_eq!(flow, "[!timestamp , null, !timestamp ]\n");
    assert_eq!(
        from_str::<Vec<Option<Tagged<Event>>>>(&flow).unwrap(),
        sequence
    );

    let mapping = BTreeMap::from([
        ("first".to_owned(), timestamp()),
        ("last".to_owned(), timestamp()),
    ]);
    let block = to_string(&mapping).unwrap();
    assert_eq!(block, "first: !timestamp\nlast: !timestamp\n");
    assert_eq!(
        from_str::<BTreeMap<String, Tagged<Event>>>(&block).unwrap(),
        mapping
    );
    let flow = to_string(&FlowMap(&mapping)).unwrap();
    assert_eq!(flow, "{first: !timestamp , last: !timestamp }\n");
    assert_eq!(
        from_str::<BTreeMap<String, Tagged<Event>>>(&flow).unwrap(),
        mapping
    );
}

#[test]
fn tag_only_unit_mapping_keys_round_trip() {
    struct Keyed;

    impl Serialize for Keyed {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut mapping = serializer.serialize_map(Some(1))?;
            mapping.serialize_entry(&timestamp(), &42)?;
            mapping.end()
        }
    }

    let yaml = to_string(&Keyed).unwrap();
    assert_eq!(yaml, "? !timestamp\n: 42\n");
    assert_eq!(
        from_str::<BTreeMap<Option<Event>, u32>>(&yaml).unwrap(),
        BTreeMap::from([(Some(Event::Timestamp), 42)]),
    );
}

#[test]
fn tag_only_unit_documents_are_not_dropped() {
    let value = timestamp();
    let yaml = to_string(&value).unwrap();
    let stream = format!("---\n{yaml}---\n{yaml}");
    let expected = vec![Some(Event::Timestamp), Some(Event::Timestamp)];
    assert_eq!(
        serde_saphyr::from_multiple::<Option<Event>>(&stream).unwrap(),
        expected
    );
    let mut reader = std::io::Cursor::new(stream.as_bytes());
    let actual: Vec<Option<Event>> = serde_saphyr::read(&mut reader)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(actual, expected);
}

/// `tag` selects `value`; `expected` is the complete scalar serialization.
/// Returns normally when a genuine variant field is preserved and round-trips.
#[rstest]
#[case("!count", Event::Count(42), "!count 42\n")]
#[case("!maybe", Event::Maybe(None), "!maybe null\n")]
#[case("!maybe", Event::Maybe(Some(42)), "!maybe 42\n")]
fn variant_fields_keep_their_payloads(
    #[case] tag: &str,
    #[case] value: Event,
    #[case] expected: &str,
) {
    let tagged = Tagged(Some(value), Some(tag.into()));
    let yaml = to_string(&tagged).unwrap();
    assert_eq!(yaml, expected);
    assert_eq!(from_str::<Tagged<Option<Event>>>(&yaml).unwrap(), tagged);
}

/// `tag` selects `value`; `marker` is the required empty collection marker.
/// Returns normally when collection payload shape is preserved through a round-trip.
#[rstest]
#[case("!empty_tuple", Event::EmptyTuple(), "[]")]
#[case("!empty_struct", Event::EmptyStruct {}, "{}")]
fn empty_collection_variants_keep_their_shape(
    #[case] tag: &str,
    #[case] value: Event,
    #[case] marker: &str,
) {
    let tagged = Tagged(value, Some(tag.into()));
    let yaml = to_string(&tagged).unwrap();
    assert!(yaml.contains(marker), "{yaml}");
    assert!(!yaml.contains("null"), "{yaml}");
    assert_eq!(from_str::<Tagged<Event>>(&yaml).unwrap(), tagged);
}

#[test]
fn an_outer_selector_does_not_erase_an_inner_unit_variant() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Outer {
        #[serde(rename = "timestamp")]
        Timestamp(Event),
    }
    let tagged = Tagged(
        Outer::Timestamp(Event::Timestamp),
        Some("!timestamp".into()),
    );
    let yaml = to_string(&tagged).unwrap();
    assert_eq!(yaml, "!timestamp timestamp\n");
    assert_eq!(from_str::<Tagged<Outer>>(&yaml).unwrap(), tagged);
}

#[test]
fn tag_only_units_keep_anchors_and_aliases() {
    use serde_saphyr::ArcAnchor;
    use std::sync::Arc;

    let anchored = ArcAnchor(Arc::new(timestamp()));
    let values = [&anchored, &anchored];
    let block = to_string(&values).unwrap();
    assert_eq!(block, "- !timestamp &a1\n- *a1\n");
    assert_eq!(
        from_str::<Vec<Tagged<Event>>>(&block).unwrap(),
        [timestamp(), timestamp()]
    );
    let flow = to_string(&FlowSeq(values)).unwrap();
    assert_eq!(flow, "[!timestamp &a1 , *a1]\n");
    assert_eq!(
        from_str::<Vec<Tagged<Event>>>(&flow).unwrap(),
        [timestamp(), timestamp()]
    );
}
