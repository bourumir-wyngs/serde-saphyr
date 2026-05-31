use std::borrow::Cow;

use granit_parser::ScalarStyle;

use super::cfg::Cfg;
use super::events::{Ev, Events, ReplayEvents, attach_alias_locations_if_missing};
use super::key_nodes::*;
use super::tags::SfTag;
use super::{DuplicateKeyPolicy, Error, Location, MergeKeyPolicy, Options};

fn loc(line: usize, column: usize) -> Location {
    Location::new(line, column)
}

fn scalar(
    value: &'static str,
    tag: SfTag,
    raw_tag: Option<&'static str>,
    style: ScalarStyle,
    location: Location,
) -> Ev<'static> {
    Ev::Scalar {
        value: Cow::Borrowed(value),
        tag,
        raw_tag: raw_tag.map(Cow::Borrowed),
        style,
        anchor: 0,
        location,
    }
}

fn seq_start(tag: SfTag, raw_tag: Option<&'static str>, location: Location) -> Ev<'static> {
    Ev::SeqStart {
        anchor: 0,
        tag,
        raw_tag: raw_tag.map(Cow::Borrowed),
        location,
    }
}

fn seq_end(location: Location) -> Ev<'static> {
    Ev::SeqEnd { location }
}

fn map_start(location: Location) -> Ev<'static> {
    Ev::MapStart {
        anchor: 0,
        location,
    }
}

fn map_end(location: Location) -> Ev<'static> {
    Ev::MapEnd { location }
}

#[cfg(not(feature = "properties"))]
fn replay_events(buf: Vec<Ev<'static>>) -> ReplayEvents<'static> {
    ReplayEvents::new(buf)
}

#[cfg(feature = "properties")]
fn replay_events(buf: Vec<Ev<'static>>) -> ReplayEvents<'static> {
    ReplayEvents::new(buf, None)
}

#[cfg(not(feature = "properties"))]
fn replay_events_with_reference(
    buf: Vec<Ev<'static>>,
    reference: Location,
) -> ReplayEvents<'static> {
    ReplayEvents::with_reference(buf, reference)
}

#[cfg(feature = "properties")]
fn replay_events_with_reference(
    buf: Vec<Ev<'static>>,
    reference: Location,
) -> ReplayEvents<'static> {
    ReplayEvents::with_reference(buf, reference, None)
}

#[cfg(not(feature = "properties"))]
fn pending_from_events(
    events: Vec<Ev<'static>>,
    location: Location,
    reference_location: Location,
) -> Result<Vec<PendingEntry<'static>>, Error> {
    pending_entries_from_events(
        events,
        location,
        reference_location,
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
    )
}

#[cfg(feature = "properties")]
fn pending_from_events(
    events: Vec<Ev<'static>>,
    location: Location,
    reference_location: Location,
) -> Result<Vec<PendingEntry<'static>>, Error> {
    pending_entries_from_events(
        events,
        location,
        reference_location,
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
        None,
    )
}

fn scalar_text(events: &[Ev<'_>]) -> Option<String> {
    match events.first() {
        Some(Ev::Scalar { value, .. }) => Some(value.as_ref().to_owned()),
        _ => None,
    }
}

fn pending_pair(entry: &PendingEntry<'_>) -> (String, String, Location) {
    (
        scalar_text(entry.key.events()).expect("scalar key"),
        scalar_text(entry.value.events()).expect("scalar value"),
        entry.reference_location,
    )
}

fn unwrap_err<T>(result: Result<T, Error>) -> Error {
    match result {
        Ok(_) => panic!("expected error"),
        Err(err) => err,
    }
}

fn scalar_key_node(
    value: &'static str,
    tag: SfTag,
    style: ScalarStyle,
    location: Location,
) -> KeyNode<'static> {
    KeyNode::Scalar {
        events: vec![scalar(value, tag, None, style, location)],
        location,
    }
}

#[test]
#[allow(deprecated)]
fn cfg_and_replay_events_follow_options_and_reference_overrides() {
    let options = Options {
        duplicate_keys: DuplicateKeyPolicy::LastWins,
        merge_keys: MergeKeyPolicy::AsOrdinary,
        legacy_octal_numbers: true,
        strict_booleans: true,
        angle_conversions: true,
        ignore_binary_tag_for_string: true,
        no_schema: true,
        ..Options::default()
    };

    let cfg = Cfg::from_options(&options);
    assert!(matches!(cfg.dup_policy, DuplicateKeyPolicy::LastWins));
    assert!(matches!(cfg.merge_keys, MergeKeyPolicy::AsOrdinary));
    assert!(cfg.legacy_octal_numbers);
    assert!(cfg.strict_booleans);
    assert!(cfg.angle_conversions);
    assert!(cfg.ignore_binary_tag_for_string);
    assert!(cfg.no_schema);

    assert_eq!(Ev::default().location(), Location::UNKNOWN);

    let value_loc = loc(1, 1);
    let alias_loc = loc(9, 9);
    let mut replay = replay_events_with_reference(
        vec![scalar(
            "value",
            SfTag::String,
            None,
            ScalarStyle::Plain,
            value_loc,
        )],
        alias_loc,
    );
    assert!(matches!(replay.peek().unwrap(), Some(Ev::Scalar { .. })));
    assert_eq!(replay.reference_location(), alias_loc);
    assert_eq!(replay.last_location(), value_loc);
    assert!(matches!(
        replay.next().unwrap(),
        Some(Ev::Scalar { location, .. }) if location == value_loc
    ));
    assert!(replay.peek().unwrap().is_none());
    assert_eq!(replay.last_location(), value_loc);
    assert_eq!(replay.reference_location(), alias_loc);

    let empty = replay_events(Vec::new());
    assert_eq!(empty.last_location(), Location::UNKNOWN);
    assert_eq!(empty.reference_location(), Location::UNKNOWN);
}

#[test]
fn attach_alias_locations_prefers_dual_locations_and_existing_errors() {
    let reference = loc(1, 2);
    let defined = loc(3, 4);
    let existing = loc(5, 6);

    let err = Error::msg("alias boom").with_location(existing);
    let expected_message = err.to_string();
    let attached = attach_alias_locations_if_missing(err, reference, defined);
    match attached {
        Error::AliasError { msg, locations } => {
            assert_eq!(msg, expected_message);
            assert_eq!(locations.reference_location, reference);
            assert_eq!(locations.defined_location, defined);
        }
        other => panic!("expected alias error, got {other:?}"),
    }

    let preserved = attach_alias_locations_if_missing(
        Error::unexpected("value").with_location(existing),
        Location::UNKNOWN,
        Location::UNKNOWN,
    );
    assert!(!matches!(&preserved, Error::AliasError { .. }));
    assert_eq!(preserved.location(), Some(existing));

    let preferred_reference =
        attach_alias_locations_if_missing(Error::msg("same"), reference, reference);
    assert_eq!(preferred_reference.location(), Some(reference));

    let fallback_defined =
        attach_alias_locations_if_missing(Error::msg("defined"), Location::UNKNOWN, defined);
    assert_eq!(fallback_defined.location(), Some(defined));
}

#[test]
fn simple_tagged_enum_helpers_accept_only_simple_variant_names() {
    assert_eq!(
        simple_tagged_enum_name(&Some(Cow::Borrowed("!Widget")), &SfTag::Other),
        Some("Widget".to_owned())
    );
    assert_eq!(
        simple_tagged_enum_name(
            &Some(Cow::Borrowed("!<tag:yaml.org,2002:Widget>")),
            &SfTag::Other
        ),
        Some("Widget".to_owned())
    );
    assert_eq!(
        simple_tagged_enum_name(&Some(Cow::Borrowed("!")), &SfTag::Other),
        None
    );
    assert_eq!(
        simple_tagged_enum_name(&Some(Cow::Borrowed("!bad:name")), &SfTag::Other),
        None
    );
    assert_eq!(
        simple_tagged_enum_name(&Some(Cow::Borrowed("!bang!oops")), &SfTag::Other),
        None
    );
    assert_eq!(
        simple_tagged_enum_name(&Some(Cow::Borrowed("!Widget")), &SfTag::String),
        None
    );

    let scalar_loc = loc(7, 1);
    let scalar_event = scalar(
        "payload",
        SfTag::Other,
        Some("!ScalarVariant"),
        ScalarStyle::Plain,
        scalar_loc,
    );
    assert_eq!(
        simple_tagged_node_name(&scalar_event),
        Some(("ScalarVariant".to_owned(), scalar_loc))
    );

    let seq_loc = loc(8, 1);
    let seq_event = seq_start(SfTag::Other, Some("!SeqVariant"), seq_loc);
    assert_eq!(
        simple_tagged_node_name(&seq_event),
        Some(("SeqVariant".to_owned(), seq_loc))
    );
    assert_eq!(simple_tagged_node_name(&map_start(loc(9, 1))), None);
}

#[test]
fn key_fingerprint_helpers_normalize_string_like_tags() {
    assert_eq!(canonical_scalar_key_tag(SfTag::None), SfTag::String);
    assert_eq!(canonical_scalar_key_tag(SfTag::Other), SfTag::String);
    assert_eq!(canonical_scalar_key_tag(SfTag::Include), SfTag::String);
    assert_eq!(canonical_scalar_key_tag(SfTag::NonSpecific), SfTag::String);
    assert_eq!(canonical_scalar_key_tag(SfTag::Int), SfTag::Int);

    let stringy = KeyFingerprint::Scalar {
        value: "hello".to_owned(),
        tag: SfTag::Other,
    };
    assert_eq!(stringy.stringy_scalar_value(), Some("hello"));

    let binary = KeyFingerprint::Scalar {
        value: "SGVsbG8=".to_owned(),
        tag: SfTag::Binary,
    };
    assert_eq!(binary.stringy_scalar_value(), None);
    assert_eq!(
        KeyFingerprint::Sequence(vec![]).stringy_scalar_value(),
        None
    );
}

#[test]
fn one_entry_map_spans_and_skip_one_node_len_handle_nested_and_malformed_inputs() {
    let events = vec![
        map_start(loc(10, 1)),
        scalar("key", SfTag::None, None, ScalarStyle::Plain, loc(10, 2)),
        seq_start(SfTag::None, None, loc(10, 3)),
        scalar("item", SfTag::None, None, ScalarStyle::Plain, loc(10, 4)),
        map_start(loc(10, 5)),
        scalar("nested", SfTag::None, None, ScalarStyle::Plain, loc(10, 6)),
        scalar("value", SfTag::None, None, ScalarStyle::Plain, loc(10, 7)),
        map_end(loc(10, 8)),
        seq_end(loc(10, 9)),
        map_end(loc(10, 10)),
    ];

    assert_eq!(skip_one_node_len(&events, 1), Some(1));
    assert_eq!(skip_one_node_len(&events, 2), Some(7));
    assert_eq!(one_entry_map_spans(&events), Some((1, 2, 2, 9)));

    assert_eq!(
        skip_one_node_len(
            &[Ev::SeqEnd {
                location: loc(11, 1)
            }],
            0
        ),
        None
    );
    assert_eq!(
        skip_one_node_len(
            &[Ev::Taken {
                location: loc(12, 1)
            }],
            0
        ),
        None
    );

    let malformed = vec![
        seq_start(SfTag::None, None, loc(13, 1)),
        scalar(
            "unterminated",
            SfTag::None,
            None,
            ScalarStyle::Plain,
            loc(13, 2),
        ),
    ];
    assert_eq!(skip_one_node_len(&malformed, 0), None);

    let extra = vec![
        map_start(loc(14, 1)),
        scalar("key", SfTag::None, None, ScalarStyle::Plain, loc(14, 2)),
        scalar("value", SfTag::None, None, ScalarStyle::Plain, loc(14, 3)),
        scalar("extra", SfTag::None, None, ScalarStyle::Plain, loc(14, 4)),
        map_end(loc(14, 5)),
    ];
    assert_eq!(one_entry_map_spans(&extra), None);
}

#[test]
fn capture_node_captures_nested_fingerprints_and_rejects_invalid_streams() {
    let start = loc(15, 1);
    let mut replay = replay_events(vec![
        seq_start(SfTag::None, None, start),
        scalar("a", SfTag::None, None, ScalarStyle::Plain, loc(15, 2)),
        map_start(loc(15, 3)),
        scalar("b", SfTag::None, None, ScalarStyle::Plain, loc(15, 4)),
        scalar("c", SfTag::None, None, ScalarStyle::Plain, loc(15, 5)),
        map_end(loc(15, 6)),
        seq_end(loc(15, 7)),
    ]);

    match capture_node(&mut replay).unwrap() {
        KeyNode::Fingerprinted {
            fingerprint,
            events,
            location,
        } => {
            assert_eq!(location, start);
            assert_eq!(events.len(), 7);
            assert_eq!(
                fingerprint,
                KeyFingerprint::Sequence(vec![
                    KeyFingerprint::Scalar {
                        value: "a".to_owned(),
                        tag: SfTag::String,
                    },
                    KeyFingerprint::Mapping(vec![(
                        KeyFingerprint::Scalar {
                            value: "b".to_owned(),
                            tag: SfTag::String,
                        },
                        KeyFingerprint::Scalar {
                            value: "c".to_owned(),
                            tag: SfTag::String,
                        },
                    )]),
                ])
            );
        }
        _ => panic!("expected fingerprinted node"),
    }

    let mut unexpected_end = replay_events(vec![map_end(loc(16, 1))]);
    let err = unwrap_err(capture_node(&mut unexpected_end));
    assert!(matches!(
        err,
        Error::UnexpectedContainerEndWhileReadingKeyNode { location } if location == loc(16, 1)
    ));

    let mut taken = replay_events(vec![Ev::Taken {
        location: loc(17, 1),
    }]);
    let err = unwrap_err(capture_node(&mut taken));
    assert_eq!(err.location(), Some(loc(17, 1)));

    let mut eof = replay_events(vec![seq_start(SfTag::None, None, loc(18, 1))]);
    let err = unwrap_err(capture_node(&mut eof));
    assert!(matches!(err, Error::Eof { location } if location == loc(18, 1)));
}

#[test]
fn tagged_payload_helpers_strip_root_tags_and_build_external_map_events() {
    let seq_loc = loc(19, 1);
    let mut seq_payload = vec![
        seq_start(SfTag::Other, Some("!SeqVariant"), seq_loc),
        scalar("item", SfTag::None, None, ScalarStyle::Plain, loc(19, 2)),
        seq_end(loc(19, 3)),
    ];
    strip_root_tag_for_externally_tagged_payload(&mut seq_payload);
    match &seq_payload[0] {
        Ev::SeqStart { tag, raw_tag, .. } => {
            assert_eq!(*tag, SfTag::None);
            assert!(raw_tag.is_none());
        }
        other => panic!("expected seq start, got {other:?}"),
    }

    let wrapped = externally_tagged_payload_as_map_events("Empty".to_owned(), seq_loc, Vec::new());
    assert!(matches!(
        &wrapped[..],
        [
            Ev::MapStart { location, .. },
            Ev::Scalar { value, tag: SfTag::String, raw_tag: None, .. },
            Ev::MapEnd { location: end_location },
        ] if *location == seq_loc && value.as_ref() == "Empty" && *end_location == seq_loc
    ));

    let scalar_loc = loc(20, 1);
    let mut replay = replay_events(vec![scalar(
        "payload",
        SfTag::Other,
        Some("!Variant"),
        ScalarStyle::Plain,
        scalar_loc,
    )]);
    let captured = capture_simple_tagged_node_as_map_events(&mut replay)
        .unwrap()
        .expect("tagged node should be converted");
    assert!(
        matches!(captured.first(), Some(Ev::MapStart { location, .. }) if *location == scalar_loc)
    );
    assert!(matches!(
        captured.get(1),
        Some(Ev::Scalar { value, tag: SfTag::String, raw_tag: None, .. }) if value.as_ref() == "Variant"
    ));
    match captured.get(2) {
        Some(Ev::Scalar {
            value,
            tag,
            raw_tag,
            location,
            ..
        }) => {
            assert_eq!(value.as_ref(), "payload");
            assert_eq!(*tag, SfTag::None);
            assert!(raw_tag.is_none());
            assert_eq!(*location, scalar_loc);
        }
        other => panic!("expected payload scalar, got {other:?}"),
    }
    assert!(matches!(
        captured.last(),
        Some(Ev::MapEnd { location }) if *location == scalar_loc
    ));

    let mut untagged = replay_events(vec![scalar(
        "payload",
        SfTag::String,
        None,
        ScalarStyle::Plain,
        scalar_loc,
    )]);
    assert!(
        capture_simple_tagged_node_as_map_events(&mut untagged)
            .unwrap()
            .is_none()
    );
}

#[test]
fn is_merge_key_requires_plain_untagged_double_angle() {
    assert!(is_merge_key(&scalar_key_node(
        "<<",
        SfTag::None,
        ScalarStyle::Plain,
        loc(21, 1)
    )));
    assert!(!is_merge_key(&scalar_key_node(
        "<<",
        SfTag::String,
        ScalarStyle::Plain,
        loc(21, 2)
    )));
    assert!(!is_merge_key(&scalar_key_node(
        "<<",
        SfTag::None,
        ScalarStyle::SingleQuoted,
        loc(21, 3)
    )));
    assert!(!is_merge_key(&KeyNode::Fingerprinted {
        fingerprint: KeyFingerprint::Default,
        events: vec![map_start(loc(21, 4)), map_end(loc(21, 5))],
        location: loc(21, 4),
    }));
}

#[test]
fn pending_entries_from_events_handles_scalars_maps_sequences_and_eof() {
    let reference = loc(22, 9);

    let null_entries = pending_from_events(
        vec![scalar(
            "null",
            SfTag::None,
            None,
            ScalarStyle::Plain,
            loc(22, 1),
        )],
        loc(22, 1),
        reference,
    )
    .unwrap();
    assert!(null_entries.is_empty());

    let err = unwrap_err(pending_from_events(
        vec![scalar(
            "42",
            SfTag::None,
            None,
            ScalarStyle::Plain,
            loc(23, 1),
        )],
        loc(23, 1),
        reference,
    ));
    assert!(matches!(
        err,
        Error::MergeValueNotMapOrSeqOfMaps { location } if location == loc(23, 1)
    ));

    let map_entries = pending_from_events(
        vec![
            map_start(loc(24, 1)),
            scalar("a", SfTag::None, None, ScalarStyle::Plain, loc(24, 2)),
            scalar("1", SfTag::None, None, ScalarStyle::Plain, loc(24, 3)),
            map_end(loc(24, 4)),
        ],
        loc(24, 1),
        reference,
    )
    .unwrap();
    assert_eq!(map_entries.len(), 1);
    assert_eq!(
        pending_pair(&map_entries[0]),
        ("a".to_owned(), "1".to_owned(), reference)
    );

    let seq_entries = pending_from_events(
        vec![
            seq_start(SfTag::None, None, loc(25, 1)),
            map_start(loc(25, 2)),
            scalar("first", SfTag::None, None, ScalarStyle::Plain, loc(25, 3)),
            scalar("1", SfTag::None, None, ScalarStyle::Plain, loc(25, 4)),
            map_end(loc(25, 5)),
            map_start(loc(26, 2)),
            scalar("second", SfTag::None, None, ScalarStyle::Plain, loc(26, 3)),
            scalar("2", SfTag::None, None, ScalarStyle::Plain, loc(26, 4)),
            map_end(loc(26, 5)),
            seq_end(loc(27, 1)),
        ],
        loc(25, 1),
        reference,
    )
    .unwrap();
    assert_eq!(seq_entries.len(), 2);
    assert_eq!(
        pending_pair(&seq_entries[0]),
        ("second".to_owned(), "2".to_owned(), reference)
    );
    assert_eq!(
        pending_pair(&seq_entries[1]),
        ("first".to_owned(), "1".to_owned(), reference)
    );

    let err = unwrap_err(pending_from_events(Vec::new(), loc(28, 1), reference));
    assert!(matches!(err, Error::Eof { location } if location == loc(28, 1)));
}

#[test]
fn pending_entries_from_live_events_handles_null_scalars_sequences_and_eof() {
    let merge_reference = loc(29, 9);

    let mut null_replay = replay_events(vec![scalar(
        "~",
        SfTag::None,
        None,
        ScalarStyle::Plain,
        loc(29, 1),
    )]);
    assert!(
        pending_entries_from_live_events(
            &mut null_replay,
            merge_reference,
            MergeKeyPolicy::Merge,
            DuplicateKeyPolicy::Error,
        )
        .unwrap()
        .is_empty()
    );

    let mut scalar_replay = replay_events(vec![scalar(
        "42",
        SfTag::None,
        None,
        ScalarStyle::Plain,
        loc(30, 1),
    )]);
    let err = unwrap_err(pending_entries_from_live_events(
        &mut scalar_replay,
        merge_reference,
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
    ));
    assert!(matches!(
        err,
        Error::MergeValueNotMapOrSeqOfMaps { location } if location == loc(30, 1)
    ));

    let mut seq_replay = replay_events(vec![
        seq_start(SfTag::None, None, loc(31, 1)),
        map_start(loc(31, 2)),
        scalar("a", SfTag::None, None, ScalarStyle::Plain, loc(31, 3)),
        scalar("1", SfTag::None, None, ScalarStyle::Plain, loc(31, 4)),
        map_end(loc(31, 5)),
        map_start(loc(32, 2)),
        scalar("b", SfTag::None, None, ScalarStyle::Plain, loc(32, 3)),
        scalar("2", SfTag::None, None, ScalarStyle::Plain, loc(32, 4)),
        map_end(loc(32, 5)),
        seq_end(loc(33, 1)),
    ]);
    let entries = pending_entries_from_live_events(
        &mut seq_replay,
        merge_reference,
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        pending_pair(&entries[0]),
        ("b".to_owned(), "2".to_owned(), loc(32, 2))
    );
    assert_eq!(
        pending_pair(&entries[1]),
        ("a".to_owned(), "1".to_owned(), loc(31, 2))
    );

    let mut empty = replay_events(Vec::new());
    let err = unwrap_err(pending_entries_from_live_events(
        &mut empty,
        merge_reference,
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
    ));
    assert!(matches!(err, Error::Eof { location } if location == Location::UNKNOWN));
}

#[test]
fn collect_entries_from_map_expands_merges_and_preserves_reference_locations() {
    let mut not_a_map = replay_events(vec![scalar(
        "oops",
        SfTag::None,
        None,
        ScalarStyle::Plain,
        loc(34, 1),
    )]);
    let err = unwrap_err(collect_entries_from_map(
        &mut not_a_map,
        loc(34, 9),
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
    ));
    assert!(matches!(
        err,
        Error::MergeValueNotMapOrSeqOfMaps { location } if location == loc(34, 1)
    ));

    let outer_reference = loc(35, 9);
    let merge_value_location = loc(35, 3);
    let mut replay = replay_events(vec![
        map_start(loc(35, 1)),
        scalar("<<", SfTag::None, None, ScalarStyle::Plain, loc(35, 2)),
        map_start(merge_value_location),
        scalar("base", SfTag::None, None, ScalarStyle::Plain, loc(35, 4)),
        scalar("1", SfTag::None, None, ScalarStyle::Plain, loc(35, 5)),
        map_end(loc(35, 6)),
        scalar("own", SfTag::None, None, ScalarStyle::Plain, loc(35, 7)),
        scalar("2", SfTag::None, None, ScalarStyle::Plain, loc(35, 8)),
        map_end(loc(35, 10)),
    ]);

    let entries = collect_entries_from_map(
        &mut replay,
        outer_reference,
        MergeKeyPolicy::Merge,
        DuplicateKeyPolicy::Error,
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        pending_pair(&entries[0]),
        ("own".to_owned(), "2".to_owned(), outer_reference)
    );
    assert_eq!(
        pending_pair(&entries[1]),
        ("base".to_owned(), "1".to_owned(), merge_value_location)
    );
}

#[test]
fn collect_entries_from_map_treats_merge_key_as_ordinary_under_policy() {
    let reference = loc(36, 9);
    let mut replay = replay_events(vec![
        map_start(loc(36, 1)),
        scalar("<<", SfTag::None, None, ScalarStyle::Plain, loc(36, 2)),
        scalar("1", SfTag::None, None, ScalarStyle::Plain, loc(36, 3)),
        map_end(loc(36, 4)),
    ]);

    let entries = collect_entries_from_map(
        &mut replay,
        reference,
        MergeKeyPolicy::AsOrdinary,
        DuplicateKeyPolicy::Error,
    )
    .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(
        pending_pair(&entries[0]),
        ("<<".to_owned(), "1".to_owned(), reference)
    );
}

#[test]
fn collect_entries_from_map_rejects_merge_key_under_error_policy() {
    let mut replay = replay_events(vec![
        map_start(loc(37, 1)),
        scalar("<<", SfTag::None, None, ScalarStyle::Plain, loc(37, 2)),
        scalar("1", SfTag::None, None, ScalarStyle::Plain, loc(37, 3)),
        map_end(loc(37, 4)),
    ]);

    let err = unwrap_err(collect_entries_from_map(
        &mut replay,
        loc(37, 9),
        MergeKeyPolicy::Error,
        DuplicateKeyPolicy::Error,
    ));

    assert!(matches!(
        err,
        Error::MergeKeyNotAllowed { location } if location == loc(37, 2)
    ));
}
