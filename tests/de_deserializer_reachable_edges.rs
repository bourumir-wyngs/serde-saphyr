#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_saphyr::{Error, TransformReason};

#[derive(Debug, PartialEq)]
struct FirstSequenceElement(u8);

impl<'de> Deserialize<'de> for FirstSequenceElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(FirstSequenceElementVisitor)
    }
}

struct FirstSequenceElementVisitor;

impl<'de> Visitor<'de> for FirstSequenceElementVisitor {
    type Value = FirstSequenceElement;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence with at least one byte")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Ok(FirstSequenceElement(
            sequence.next_element()?.expect("fixture has a first item"),
        ))
    }
}

#[test]
fn early_sequence_return_drains_nested_remaining_nodes() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Document {
        head: FirstSequenceElement,
        tail: u8,
    }

    let document: Document =
        serde_saphyr::from_str("head: [1, [2, {deep: [3, 4]}], {another: 5}]\ntail: 9\n").unwrap();

    assert_eq!(
        document,
        Document {
            head: FirstSequenceElement(1),
            tail: 9,
        }
    );
}

#[derive(Debug, PartialEq)]
struct TupleSizeHint(u8);

impl<'de> Deserialize<'de> for TupleSizeHint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(1, TupleSizeHintVisitor)
    }
}

struct TupleSizeHintVisitor;

impl<'de> Visitor<'de> for TupleSizeHintVisitor {
    type Value = TupleSizeHint;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a one-element tuple")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        assert_eq!(sequence.size_hint(), None);
        Ok(TupleSizeHint(
            sequence.next_element()?.expect("fixture has one item"),
        ))
    }
}

#[test]
fn tuple_access_forwards_the_underlying_size_hint() {
    assert_eq!(
        serde_saphyr::from_str::<TupleSizeHint>("[7]").unwrap(),
        TupleSizeHint(7)
    );
}

#[test]
fn typeless_plus_prefixed_integer_is_numeric() {
    let value: serde_json::Value = serde_saphyr::from_str("+7").unwrap();
    assert_eq!(value, serde_json::json!(7));
}

#[test]
fn strict_boolean_false_takes_the_false_branch() {
    let options = serde_saphyr::options! { strict_booleans: true };
    assert!(!serde_saphyr::from_str_with_options::<bool>("false", options).unwrap());
}

#[derive(Debug, PartialEq)]
struct OwnedFromDeserializeStr(String);

impl<'de> Deserialize<'de> for OwnedFromDeserializeStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(OwnedStringVisitor)
    }
}

struct OwnedStringVisitor;

impl<'de> Visitor<'de> for OwnedStringVisitor {
    type Value = OwnedFromDeserializeStr;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("text, including transformed owned text")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(OwnedFromDeserializeStr(value.to_owned()))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(OwnedFromDeserializeStr(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(OwnedFromDeserializeStr(value))
    }
}

#[derive(Debug)]
struct RejectOwnedDeserializeStr;

impl<'de> Deserialize<'de> for RejectOwnedDeserializeStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RejectOwnedStringVisitor)
    }
}

struct RejectOwnedStringVisitor;

impl<'de> Visitor<'de> for RejectOwnedStringVisitor {
    type Value = RejectOwnedDeserializeStr;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("borrowed text only")
    }

    fn visit_borrowed_str<E>(self, _value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectOwnedDeserializeStr)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectOwnedDeserializeStr)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("owned strings are deliberately rejected"))
    }
}

#[test]
fn deserialize_str_handles_owned_binary_success_and_custom_visitor_error() {
    let value: OwnedFromDeserializeStr = serde_saphyr::from_str("!!binary SGVsbG8=").unwrap();
    assert_eq!(value, OwnedFromDeserializeStr("Hello".to_owned()));

    let error =
        serde_saphyr::from_str::<RejectOwnedDeserializeStr>("!!binary SGVsbG8=").unwrap_err();
    assert!(matches!(error.without_snippet(), Error::Message { .. }));
}

#[test]
fn borrowed_str_reports_binary_transform_non_string_tag_and_eof() {
    let transformed = serde_saphyr::from_str::<&str>("!!binary SGVsbG8=").unwrap_err();
    assert!(matches!(
        transformed.without_snippet(),
        Error::CannotBorrowTransformedString {
            reason: TransformReason::ParserReturnedOwned,
            ..
        }
    ));

    let tagged = serde_saphyr::from_str::<&str>("!!int 7").unwrap_err();
    assert!(matches!(
        tagged.without_snippet(),
        Error::TaggedScalarCannotDeserializeIntoString { .. }
    ));

    let eof = serde_saphyr::from_str::<&str>("").unwrap_err();
    assert!(matches!(eof.without_snippet(), Error::Eof { .. }));
}

#[test]
fn binary_that_is_not_utf8_reports_the_specific_string_error() {
    let error = serde_saphyr::from_str::<String>("!!binary /w==").unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::BinaryNotUtf8 { .. }
    ));
}

#[derive(Debug)]
struct ValueBeforeKey;

impl<'de> Deserialize<'de> for ValueBeforeKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ValueBeforeKeyVisitor)
    }
}

struct ValueBeforeKeyVisitor;

impl<'de> Visitor<'de> for ValueBeforeKeyVisitor {
    type Value = ValueBeforeKey;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a map access that rejects value-before-key misuse")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let _: IgnoredAny = map.next_value()?;
        Ok(ValueBeforeKey)
    }
}

#[test]
fn both_empty_map_access_implementations_reject_value_before_key() {
    for yaml in ["{}", "null"] {
        let error = serde_saphyr::from_str::<ValueBeforeKey>(yaml).unwrap_err();
        assert!(matches!(
            error.without_snippet(),
            Error::ValueRequestedBeforeKey { .. }
        ));
    }
}

#[derive(Debug, Deserialize, PartialEq)]
enum EnumEdges {
    Unit,
    New(u8),
    Nested(Vec<Vec<u8>>),
    Struct { value: u8 },
}

#[derive(Debug, Deserialize, PartialEq)]
enum TaggedChoice {
    Variant(String),
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum UntaggedChoice {
    Tagged(TaggedChoice),
    Plain(String),
}

#[test]
fn serde_untagged_buffer_preserves_a_simple_yaml_tag_as_an_enum_variant() {
    let value: UntaggedChoice = serde_saphyr::from_str("!Variant payload").unwrap();
    assert_eq!(
        value,
        UntaggedChoice::Tagged(TaggedChoice::Variant("payload".to_owned()))
    );
}

#[test]
fn enum_mapping_rejects_non_string_key_extra_entry_and_unit_payload() {
    let non_string_key = serde_saphyr::from_str::<EnumEdges>("? [New]\n: 1\n").unwrap_err();
    assert!(matches!(
        non_string_key.without_snippet(),
        Error::ExpectedStringKeyForExternallyTaggedEnum { .. }
    ));

    let extra_entry = serde_saphyr::from_str::<EnumEdges>("New: 1\nUnit: null\n").unwrap_err();
    assert!(matches!(
        extra_entry.without_snippet(),
        Error::ExpectedMappingEndAfterEnumVariantValue { .. }
    ));

    let unit_payload = serde_saphyr::from_str::<EnumEdges>("Unit: 1\n").unwrap_err();
    assert!(matches!(
        unit_payload.without_snippet(),
        Error::UnexpectedValueForUnitEnumVariant { .. }
    ));
}

#[test]
fn tagged_sequence_enum_captures_nested_containers_and_dispatches_variants() {
    let nested: EnumEdges = serde_saphyr::from_str("!Nested [[1, 2], [3]]").unwrap();
    assert_eq!(nested, EnumEdges::Nested(vec![vec![1, 2], vec![3]]));

    let unit: EnumEdges = serde_saphyr::from_str("!Unit []").unwrap();
    assert_eq!(unit, EnumEdges::Unit);

    let wrong_struct_shape = serde_saphyr::from_str::<EnumEdges>("!Struct [value, 1]").unwrap_err();
    assert!(matches!(
        wrong_struct_shape.without_snippet(),
        Error::Unexpected { .. }
    ));
}

#[test]
fn first_wins_validates_nested_merge_keys_in_a_skipped_value() {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::FirstWins,
        merge_keys: serde_saphyr::MergeKeyPolicy::Error,
    };
    let error = serde_saphyr::from_str_with_options::<BTreeMap<String, serde_json::Value>>(
        "value: keep\nvalue: {nested: {<<: {admin: true}}}\n",
        options,
    )
    .unwrap_err();

    assert!(matches!(
        error.without_snippet(),
        Error::MergeKeyNotAllowed { .. }
    ));
}
