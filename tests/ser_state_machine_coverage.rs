#![cfg(feature = "serialize")]

use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use serde_saphyr::{
    ArcRecursion, ArcRecursive, CommentPosition, Commented, FoldStr, RcAnchor, to_fmt_writer,
    to_string, to_string_multiple_with_options, to_string_with_options,
};

#[derive(Serialize)]
#[serde(rename = "__yaml_commented")]
struct CommentWithI128(i128, &'static str);

#[test]
fn commented_internal_payload_rejects_i128_comment() {
    let err = to_string(&CommentWithI128(1, "value")).unwrap_err();

    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Unexpected { .. }
    ));
}

#[derive(Serialize)]
struct PairKey(u8, u8);

#[derive(Serialize)]
enum CompositeKey {
    Tuple(u8, u8),
    Struct { value: u8 },
}

struct MappingKey;

impl Serialize for MappingKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("nested", &7)?;
        map.end()
    }
}

struct EveryCompositeKeyShape;

impl Serialize for EveryCompositeKeyShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(5))?;
        map.serialize_entry(&(1_u8, 2_u8), "tuple")?;
        map.serialize_entry(&PairKey(3, 4), "tuple struct")?;
        map.serialize_entry(&CompositeKey::Tuple(5, 6), "tuple variant")?;
        map.serialize_entry(&MappingKey, "mapping")?;
        map.serialize_entry(&CompositeKey::Struct { value: 8 }, "struct variant")?;
        map.end()
    }
}

#[test]
fn all_composite_key_entrypoints_fall_back_to_explicit_keys() {
    let yaml = to_string(&EveryCompositeKeyShape).unwrap();

    for value in [
        "tuple",
        "tuple struct",
        "tuple variant",
        "mapping",
        "struct variant",
    ] {
        assert!(yaml.contains(value), "missing {value:?} in:\n{yaml}");
    }
    assert_eq!(yaml.matches("? ").count(), 5, "unexpected YAML:\n{yaml}");
}

#[derive(Serialize)]
struct DeepCompositeKey {
    items: Vec<BTreeMap<Vec<u8>, &'static str>>,
}

#[test]
fn deeply_indented_composite_key_aligns_its_value_after_a_dash() {
    let value = DeepCompositeKey {
        items: vec![BTreeMap::from([(vec![1, 2], "value")])],
    };
    let options = serde_saphyr::ser_options! {
        compact_list_indent: false,
    };

    let yaml = to_string_with_options(&value, options).unwrap();

    assert!(
        yaml.contains("? - 1"),
        "missing explicit sequence key:\n{yaml}"
    );
    assert!(yaml.contains(": value"), "missing aligned value:\n{yaml}");
}

#[derive(Serialize)]
struct AnchoredEmptyMap {
    items: Vec<RcAnchor<BTreeMap<String, u8>>>,
}

#[test]
fn anchored_empty_map_after_an_indented_dash_uses_aligned_braces() {
    let value = AnchoredEmptyMap {
        items: vec![RcAnchor::wrapping(BTreeMap::new())],
    };
    let options = serde_saphyr::ser_options! {
        compact_list_indent: false,
    };

    let yaml = to_string_with_options(&value, options).unwrap();

    assert!(yaml.contains("- &a1\n"), "missing anchor:\n{yaml}");
    assert!(
        yaml.contains("    {}\n"),
        "empty map was not aligned:\n{yaml}"
    );
}

#[derive(Serialize)]
struct CommentedAlias {
    definition: RcAnchor<i32>,
    alias: Commented<RcAnchor<i32>>,
}

#[test]
fn above_comment_places_an_alias_at_the_start_of_its_own_line() {
    let shared = Rc::new(7);
    let value = CommentedAlias {
        definition: RcAnchor(shared.clone()),
        alias: Commented(RcAnchor(shared), "reused".to_owned()),
    };
    let options = serde_saphyr::ser_options! {
        comment_position: CommentPosition::Above,
    };

    let yaml = to_string_with_options(&value, options).unwrap();

    assert_eq!(yaml, "definition: &a1 7\nalias:\n  # reused\n  *a1\n");
}

#[derive(Serialize)]
enum AnchoredVariant {
    Newtype(u8),
    Tuple(u8, u8),
    Struct { value: u8 },
}

#[test]
fn anchors_break_the_line_before_each_enum_payload_shape() {
    let values = vec![
        RcAnchor::wrapping(AnchoredVariant::Newtype(1)),
        RcAnchor::wrapping(AnchoredVariant::Tuple(2, 3)),
        RcAnchor::wrapping(AnchoredVariant::Struct { value: 4 }),
    ];

    let yaml = to_string(&values).unwrap();

    for expected in ["Newtype", "Tuple", "Struct"] {
        assert!(yaml.contains(expected), "missing {expected} in:\n{yaml}");
    }
    assert_eq!(yaml.matches('&').count(), 3, "unexpected anchors:\n{yaml}");
}

#[derive(Default)]
struct RejectFoldedBody {
    prefix: String,
}

impl fmt::Write for RejectFoldedBody {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.contains("REJECT_THIS_FOLDED_BODY") {
            return Err(fmt::Error);
        }
        self.prefix.push_str(text);
        Ok(())
    }
}

#[test]
fn folded_body_writer_failure_is_propagated() {
    let mut writer = RejectFoldedBody::default();
    let value = FoldStr(
        "REJECT_THIS_FOLDED_BODY is deliberately long enough to use folded scalar formatting",
    );

    let err = to_fmt_writer(&mut writer, &value).unwrap_err();

    assert!(matches!(err, serde_saphyr::ser_error::Error::Format { .. }));
    assert!(
        writer.prefix.starts_with(">\n"),
        "prefix: {:?}",
        writer.prefix
    );
}

struct SometimesFails(bool);

impl Serialize for SometimesFails {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.0 {
            Err(serde::ser::Error::custom(
                "deliberate serialization failure",
            ))
        } else {
            serializer.serialize_u8(1)
        }
    }
}

#[test]
fn string_with_options_propagates_a_value_error() {
    let err =
        to_string_with_options(&SometimesFails(true), serde_saphyr::ser_options! {}).unwrap_err();

    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { .. }
    ));
}

#[test]
fn multiple_documents_propagate_a_later_value_error() {
    let values = [SometimesFails(false), SometimesFails(true)];
    let err = to_string_multiple_with_options(&values, serde_saphyr::ser_options! {}).unwrap_err();

    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { .. }
    ));
}

fn poisoned_recursive_value() -> Arc<Mutex<Option<i32>>> {
    let value = Arc::new(Mutex::new(Some(9)));
    let poison = Arc::clone(&value);
    let _ = std::thread::spawn(move || {
        let _guard = poison.lock().unwrap();
        panic!("poison recursive anchor mutex for coverage");
    })
    .join();
    value
}

#[test]
#[cfg_attr(not(panic = "unwind"), ignore = "Test requires panic unwinding")]
fn poisoned_arc_recursive_definition_reports_a_serde_error() {
    let err = to_string(&ArcRecursive(poisoned_recursive_value())).unwrap_err();

    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { .. }
    ));
}

#[test]
#[cfg_attr(not(panic = "unwind"), ignore = "Test requires panic unwinding")]
fn poisoned_arc_recursive_reference_reports_a_serde_error() {
    let value = poisoned_recursive_value();
    let reference = ArcRecursion(Arc::downgrade(&value));
    let err = to_string(&reference).unwrap_err();

    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { .. }
    ));
}

struct ScientificKey;

impl Serialize for ScientificKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(1.234_567_890_123e100)
    }
}

struct ScientificKeyMap;

impl Serialize for ScientificKeyMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&ScientificKey, &"value")?;
        map.end()
    }
}

#[test]
fn scientific_float_key_preserves_a_fractional_mantissa() {
    let yaml = to_string(&ScientificKeyMap).unwrap();

    assert!(
        yaml.starts_with("1.234567890123e+100: value"),
        "yaml: {yaml}"
    );
}
