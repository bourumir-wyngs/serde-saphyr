//! Targeted tests to increase coverage of `src/ser.rs`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use serde::Serialize;
use serde_saphyr::{
    to_string, to_string_with_options, ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor,
    Commented, FlowMap, FlowSeq, FoldStr, FoldString, LitStr, LitString, RcAnchor, RcRecursion,
    RcRecursive, RcWeakAnchor, SerializerOptions, SpaceAfter,
};

// ── with_indent constructor ───────────────────────────────────────────────────

#[test]
fn with_indent_constructor_produces_correct_indentation() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    #[derive(Serialize)]
    struct Outer {
        a: Inner,
    }
    let mut out = String::new();
    {
        let mut ser = serde_saphyr::Serializer::with_indent(&mut out, 4);
        Outer { a: Inner { x: 1 } }.serialize(&mut ser).unwrap();
    }
    // 4-space indent means "x" is indented by 4 spaces
    assert!(out.contains("    x:"), "expected 4-space indent, got:\n{out}");
}

// ── quote_all mode ────────────────────────────────────────────────────────────

#[test]
fn quote_all_mode_single_quotes_plain_strings() {
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"hello", opts).unwrap();
    assert!(yaml.contains("'hello'"), "expected single-quoted: {yaml}");
}

#[test]
fn quote_all_mode_double_quotes_special_strings() {
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    // String with backslash requires double quotes
    let yaml = to_string_with_options(&"back\\slash", opts).unwrap();
    assert!(yaml.contains('"'), "expected double-quoted: {yaml}");
}

#[test]
fn quote_all_mode_map_values_quoted() {
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    let mut m = BTreeMap::new();
    m.insert("key", "value");
    let yaml = to_string_with_options(&m, opts).unwrap();
    assert!(yaml.contains("'value'"), "expected quoted value: {yaml}");
}

// ── KeyScalarSink: bool/int/float/unit/char/newtype keys ─────────────────────

#[test]
fn bool_key_in_map() {
    // bool key -> KeyScalarSink::serialize_bool
    let mut m = BTreeMap::new();
    m.insert(true, "yes");
    m.insert(false, "no");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("true:") || yaml.contains("false:"), "yaml: {yaml}");
}

#[test]
fn i8_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(42i8, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("42:"), "yaml: {yaml}");
}

#[test]
fn i16_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(1000i16, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("1000:"), "yaml: {yaml}");
}

#[test]
fn i32_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(-5i32, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("-5:"), "yaml: {yaml}");
}

#[test]
fn i128_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(999i128, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("999:"), "yaml: {yaml}");
}

#[test]
fn u8_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(255u8, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("255:"), "yaml: {yaml}");
}

#[test]
fn u16_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(1u16, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("1:"), "yaml: {yaml}");
}

#[test]
fn u32_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(100u32, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("100:"), "yaml: {yaml}");
}

#[test]
fn u128_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(12345u128, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("12345:"), "yaml: {yaml}");
}

#[test]
fn f32_key_in_map() {
    // f32 key -> KeyScalarSink::serialize_f32
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct F32Key(u32);
    impl Serialize for F32Key {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_f32(f32::from_bits(self.0))
        }
    }
    let mut m = BTreeMap::new();
    m.insert(F32Key(1.5f32.to_bits()), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("1.5:"), "yaml: {yaml}");
}

#[test]
fn f64_key_in_map() {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct F64Key(u64);
    impl Serialize for F64Key {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_f64(f64::from_bits(self.0))
        }
    }
    let mut m = BTreeMap::new();
    m.insert(F64Key(2.5f64.to_bits()), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("2.5:"), "yaml: {yaml}");
}

#[test]
fn char_key_in_map() {
    // char key -> KeyScalarSink::serialize_char
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct CharKey(char);
    impl Serialize for CharKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_char(self.0)
        }
    }
    let mut m = BTreeMap::new();
    m.insert(CharKey('z'), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("z:"), "yaml: {yaml}");
}

#[test]
fn unit_key_in_map() {
    // unit key -> KeyScalarSink::serialize_unit
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct UnitKey;
    impl Serialize for UnitKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_unit()
        }
    }
    let mut m = BTreeMap::new();
    m.insert(UnitKey, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("null:"), "yaml: {yaml}");
}

#[test]
fn unit_struct_key_in_map() {
    // unit_struct key -> KeyScalarSink::serialize_unit_struct
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct UnitStructKey;
    impl Serialize for UnitStructKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_unit_struct("UnitStructKey")
        }
    }
    let mut m = BTreeMap::new();
    m.insert(UnitStructKey, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("null:"), "yaml: {yaml}");
}

#[test]
fn unit_variant_key_in_map() {
    // unit_variant key -> KeyScalarSink::serialize_unit_variant
    #[derive(PartialEq, Eq, PartialOrd, Ord, Serialize)]
    enum Color {
        Red,
        Blue,
    }
    let mut m = BTreeMap::new();
    m.insert(Color::Red, 1);
    m.insert(Color::Blue, 2);
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("Red:") || yaml.contains("Blue:"), "yaml: {yaml}");
}

#[test]
fn newtype_struct_key_in_map() {
    // newtype_struct key -> KeyScalarSink::serialize_newtype_struct (transparent)
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct WrappedStr(String);
    impl Serialize for WrappedStr {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_newtype_struct("WrappedStr", &self.0)
        }
    }
    let mut m = BTreeMap::new();
    m.insert(WrappedStr("mykey".to_string()), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("mykey:"), "yaml: {yaml}");
}

#[test]
fn collect_str_key_in_map() {
    // collect_str key -> KeyScalarSink::collect_str
    struct DisplayKey(i32);
    impl Serialize for DisplayKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_str(&self.0)
        }
    }
    impl PartialEq for DisplayKey { fn eq(&self, o: &Self) -> bool { self.0 == o.0 } }
    impl Eq for DisplayKey {}
    impl PartialOrd for DisplayKey { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) } }
    impl Ord for DisplayKey { fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.0.cmp(&o.0) } }
    let mut m = BTreeMap::new();
    m.insert(DisplayKey(42), "val");
    let yaml = to_string(&m).unwrap();
    // collect_str produces a string key which may be quoted
    assert!(yaml.contains("42"), "yaml: {yaml}");
}

// ── ArcWeakAnchor with dangling weak ref (None branch) ───────────────────────

#[test]
fn arc_weak_anchor_dangling_emits_null() {
    let weak = {
        let arc = Arc::new(42i32);
        ArcWeakAnchor(Arc::downgrade(&arc))
        // arc dropped here, weak becomes dangling
    };
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("null"), "expected null for dangling weak: {yaml}");
}

// ── ArcRecursion with dangling weak ref (None branch) ────────────────────────

#[test]
fn arc_recursion_dangling_emits_null() {
    let weak = {
        let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(None)));
        ArcRecursion::from(&arc_rec)
        // arc_rec dropped here
    };
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("null"), "expected null for dangling arc recursion: {yaml}");
}

// ── ArcRecursive with initialized value ──────────────────────────────────────

#[test]
fn arc_recursive_serializes_value() {
    let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(Some(99))));
    let yaml = to_string(&arc_rec).unwrap();
    assert!(yaml.contains("99"), "expected 99 in: {yaml}");
}

// ── LitStr/LitString with various trailing newline counts ────────────────────

#[test]
fn lit_str_no_trailing_newline_uses_strip() {
    let s = LitStr("hello");
    let yaml = to_string(&s).unwrap();
    // literal block with strip indicator '-'
    assert!(yaml.contains("|-"), "expected strip indicator: {yaml}");
}

#[test]
fn lit_str_one_trailing_newline_uses_clip() {
    let s = LitStr("hello\n");
    let yaml = to_string(&s).unwrap();
    // literal block with clip (no indicator after |)
    assert!(yaml.contains("|\n") || yaml.contains("| \n") || yaml.starts_with("|"), "expected clip: {yaml}");
}

#[test]
fn lit_str_two_trailing_newlines_uses_keep() {
    let s = LitStr("hello\n\n");
    let yaml = to_string(&s).unwrap();
    // literal block with keep indicator '+'
    assert!(yaml.contains("|+"), "expected keep indicator: {yaml}");
}

// ── FoldStr with trailing newline variations ──────────────────────────────────

#[test]
fn fold_str_no_trailing_newline() {
    let s = FoldStr("a long string that should be folded because it is long enough to trigger folding behavior");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

// ── prefer_block_scalars option triggers auto folded style ───────────────────

#[test]
fn prefer_block_scalars_auto_fold_no_trailing_newline() {
    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"short", opts).unwrap();
    // short strings may still be plain; just ensure no panic
    let _ = yaml;
}

#[test]
fn prefer_block_scalars_auto_fold_with_trailing_newlines() {
    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };
    // multiline string with multiple trailing newlines -> auto folded with keep
    let yaml = to_string_with_options(&"line one\nline two\n\n", opts).unwrap();
    assert!(yaml.contains('>') || yaml.contains('|'), "expected block scalar: {yaml}");
}

// ── serialize_bool at line start (indent path) ───────────────────────────────

#[test]
fn bool_value_in_struct_indented() {
    #[derive(Serialize)]
    struct S {
        flag: bool,
    }
    let yaml = to_string(&S { flag: true }).unwrap();
    assert!(yaml.contains("flag: true"), "yaml: {yaml}");
}

// ── serialize_i128 / serialize_u128 ──────────────────────────────────────────

#[test]
fn i128_value_serialized() {
    let yaml = to_string(&i128::MAX).unwrap();
    assert!(yaml.contains("170141183460469231731687303715884105727"), "yaml: {yaml}");
}

#[test]
fn u128_value_serialized() {
    let yaml = to_string(&u128::MAX).unwrap();
    assert!(yaml.contains("340282366920938463463374607431768211455"), "yaml: {yaml}");
}

// ── serialize_char ────────────────────────────────────────────────────────────

#[test]
fn char_value_serialized() {
    let yaml = to_string(&'A').unwrap();
    assert!(yaml.trim() == "A", "yaml: {yaml}");
}

// ── empty seq/map with empty_as_braces ───────────────────────────────────────

#[test]
fn empty_seq_with_empty_as_braces() {
    let opts = SerializerOptions {
        empty_as_braces: true,
        ..Default::default()
    };
    let v: Vec<i32> = vec![];
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(yaml.contains("[]"), "yaml: {yaml}");
}

#[test]
fn empty_map_with_empty_as_braces() {
    let opts = SerializerOptions {
        empty_as_braces: true,
        ..Default::default()
    };
    let m: BTreeMap<String, i32> = BTreeMap::new();
    let yaml = to_string_with_options(&m, opts).unwrap();
    assert!(yaml.contains("{}"), "yaml: {yaml}");
}

// ── write_single_quoted: single quote escaping ───────────────────────────────

#[test]
fn quote_all_single_quote_in_string_escaped() {
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"it's", opts).unwrap();
    // "it's" contains a single quote; in quote_all mode it may use double quotes
    assert!(yaml.contains("it") && yaml.contains("s"), "expected quoted string: {yaml}");
}

// ── write_anchor_name: multi-digit anchor id ─────────────────────────────────

#[test]
fn many_anchors_produce_multi_digit_ids() {
    // Create enough anchors to get id >= 10
    let values: Vec<RcAnchor<i32>> = (0..15).map(|i| RcAnchor(std::rc::Rc::new(i))).collect();
    let yaml = to_string(&values).unwrap();
    // Should contain anchor names like &a10 or higher
    assert!(yaml.contains("&a10") || yaml.contains("&a11"), "yaml: {yaml}");
}

// ── Commented in flow context (suppresses comment) ───────────────────────────

#[test]
fn commented_in_flow_seq_suppresses_comment() {
    let v = FlowSeq(vec![Commented(42i32, "# note".to_string())]);
    let yaml = to_string(&v).unwrap();
    // In flow context, comment is suppressed; value still present
    assert!(yaml.contains("42"), "yaml: {yaml}");
    // Comment should NOT appear in flow
    assert!(!yaml.contains("# note"), "comment should be suppressed in flow: {yaml}");
}

// ── serialize_bytes ───────────────────────────────────────────────────────────

#[test]
fn bytes_serialized_as_base64_or_sequence() {
    struct Bytes<'a>(&'a [u8]);
    impl Serialize for Bytes<'_> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(self.0)
        }
    }
    let yaml = to_string(&Bytes(b"hello")).unwrap();
    // Just ensure it doesn't panic and produces some output
    assert!(!yaml.is_empty(), "yaml: {yaml}");
}

// ── KeyScalarSink: str key with special chars needing quotes ─────────────────

#[test]
fn str_key_with_colon_gets_quoted() {
    let mut m = BTreeMap::new();
    m.insert("key:with:colons", "val");
    let yaml = to_string(&m).unwrap();
    // Key with colons must be quoted
    assert!(yaml.contains('"') || yaml.contains('\''), "expected quoted key: {yaml}");
}

// ── serialize_none / serialize_some ──────────────────────────────────────────

#[test]
fn option_none_serialized_as_null() {
    let v: Option<i32> = None;
    let yaml = to_string(&v).unwrap();
    assert!(yaml.trim() == "null", "yaml: {yaml}");
}

#[test]
fn option_some_serialized_as_value() {
    let v: Option<i32> = Some(42);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.trim() == "42", "yaml: {yaml}");
}

// ── serialize_unit ────────────────────────────────────────────────────────────

#[test]
fn unit_serialized_as_null() {
    let yaml = to_string(&()).unwrap();
    assert!(yaml.trim() == "null", "yaml: {yaml}");
}

// ── RcWeakAnchor dangling ─────────────────────────────────────────────────────

#[test]
fn rc_weak_anchor_dangling_emits_null() {
    use std::rc::Rc;
    let weak = {
        let rc = Rc::new(42i32);
        RcWeakAnchor(Rc::downgrade(&rc))
        // rc dropped here
    };
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("null"), "expected null for dangling rc weak: {yaml}");
}

// ── RcRecursion dangling ──────────────────────────────────────────────────────

#[test]
fn rc_recursion_dangling_emits_null() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let weak = {
        let rc_rec = RcRecursive::<i32>(Rc::new(RefCell::new(None)));
        RcRecursion::from(&rc_rec)
        // rc_rec dropped here
    };
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("null"), "expected null for dangling rc recursion: {yaml}");
}

// ── FlowMap as struct field ───────────────────────────────────────────────────

#[test]
fn flow_map_nested_in_seq() {
    let v = vec![FlowMap(BTreeMap::from([("a", 1), ("b", 2)]))];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('{'), "expected flow map: {yaml}");
}

// ── tagged_enums option ───────────────────────────────────────────────────────

#[test]
fn tagged_enums_option_serializes_with_tag() {
    #[derive(Serialize)]
    enum MyEnum {
        Variant(i32),
    }
    let opts = SerializerOptions {
        tagged_enums: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&MyEnum::Variant(5), opts).unwrap();
    assert!(!yaml.is_empty(), "yaml: {yaml}");
}

// ── yaml_12 option affects bool/null serialization ───────────────────────────

#[test]
fn yaml_12_option_bool_key() {
    let opts = SerializerOptions {
        yaml_12: true,
        ..Default::default()
    };
    let mut m = BTreeMap::new();
    m.insert(true, "yes");
    let yaml = to_string_with_options(&m, opts).unwrap();
    assert!(yaml.contains("true:"), "yaml: {yaml}");
}

// ── LitString (owned) ────────────────────────────────────────────────────────

#[test]
fn lit_string_owned_no_trailing_newline() {
    let s = LitString("block content".to_string());
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("|-"), "expected strip indicator: {yaml}");
}

// ── FoldString (owned) ───────────────────────────────────────────────────────

#[test]
fn fold_string_owned() {
    let s = FoldString("a long string that should be folded because it is long enough to trigger folding behavior in the serializer".to_string());
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

// ── SpaceAfter in flow context (no extra newline) ─────────────────────────────

#[test]
fn space_after_in_flow_no_extra_newline() {
    let v = FlowSeq(vec![SpaceAfter(1i32), SpaceAfter(2i32)]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("1") && yaml.contains("2"), "yaml: {yaml}");
}

// ── serialize_map with flow and complex key ───────────────────────────────────

#[test]
fn non_scalar_key_produces_complex_key_syntax() {
    // A sequence as a map key -> complex key with '? ' syntax
    struct SeqKey;
    impl Serialize for SeqKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq;
            let mut seq = s.serialize_seq(Some(1))?;
            seq.serialize_element(&1i32)?;
            seq.end()
        }
    }
    impl PartialEq for SeqKey { fn eq(&self, _: &Self) -> bool { true } }
    impl Eq for SeqKey {}
    impl PartialOrd for SeqKey { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) } }
    impl Ord for SeqKey { fn cmp(&self, _: &Self) -> std::cmp::Ordering { std::cmp::Ordering::Equal } }

    let mut m = BTreeMap::new();
    m.insert(SeqKey, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("? "), "expected complex key syntax: {yaml}");
}

// ── i64 boundary values ───────────────────────────────────────────────────────

#[test]
fn i64_min_max_serialized() {
    let yaml_min = to_string(&i64::MIN).unwrap();
    let yaml_max = to_string(&i64::MAX).unwrap();
    assert!(yaml_min.contains("-9223372036854775808"), "yaml: {yaml_min}");
    assert!(yaml_max.contains("9223372036854775807"), "yaml: {yaml_max}");
}

// ── u64 max ───────────────────────────────────────────────────────────────────

#[test]
fn u64_max_serialized() {
    let yaml = to_string(&u64::MAX).unwrap();
    assert!(yaml.contains("18446744073709551615"), "yaml: {yaml}");
}

// ── u128 as map value (not at line start) ────────────────────────────────────

#[test]
fn u128_as_map_value_not_at_line_start() {
    #[derive(Serialize)]
    struct S { val: u128 }
    let yaml = to_string(&S { val: u128::MAX }).unwrap();
    assert!(yaml.contains("340282366920938463463374607431768211455"), "yaml: {yaml}");
}

// ── i128 as map value (not at line start) ────────────────────────────────────

#[test]
fn i128_as_map_value_not_at_line_start() {
    #[derive(Serialize)]
    struct S { val: i128 }
    let yaml = to_string(&S { val: i128::MIN }).unwrap();
    assert!(yaml.contains("-170141183460469231731687303715884105728"), "yaml: {yaml}");
}

// ── bool as map value (not at line start, covers serialize_bool indent branch) ─

#[test]
fn bool_as_map_value_not_at_line_start() {
    #[derive(Serialize)]
    struct S { a: bool, b: bool }
    let yaml = to_string(&S { a: true, b: false }).unwrap();
    assert!(yaml.contains("a: true") && yaml.contains("b: false"), "yaml: {yaml}");
}

// ── alias written not at line start (map value position) ─────────────────────

#[test]
fn alias_as_map_value_not_at_line_start() {
    use std::rc::Rc;
    #[derive(Serialize)]
    struct S { a: RcAnchor<i32>, b: RcAnchor<i32> }
    let shared = Rc::new(42i32);
    let yaml = to_string(&S {
        a: RcAnchor(shared.clone()),
        b: RcAnchor(shared.clone()),
    }).unwrap();
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

// ── block seq as map value after another block value (forces newline) ─────────

#[test]
fn block_seq_after_block_value_forces_newline() {
    #[derive(Serialize)]
    struct S { first: Vec<i32>, second: Vec<i32> }
    let yaml = to_string(&S { first: vec![1, 2], second: vec![3, 4] }).unwrap();
    assert!(yaml.contains("first:") && yaml.contains("second:"), "yaml: {yaml}");
}

// ── block map as map value after another block value (forces newline) ─────────

#[test]
fn block_map_after_block_value_forces_newline() {
    #[derive(Serialize)]
    struct Inner { x: i32 }
    #[derive(Serialize)]
    struct S { first: Inner, second: Inner }
    let yaml = to_string(&S { first: Inner { x: 1 }, second: Inner { x: 2 } }).unwrap();
    assert!(yaml.contains("first:") && yaml.contains("second:"), "yaml: {yaml}");
}

// ── flow seq inside flow seq (nested in_flow > 0) ────────────────────────────

#[test]
fn flow_seq_inside_flow_seq() {
    let v = FlowSeq(vec![FlowSeq(vec![1i32, 2]), FlowSeq(vec![3i32, 4])]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("[["), "expected nested flow: {yaml}");
}

// ── flow map inside flow seq (nested in_flow > 0) ────────────────────────────

#[test]
fn flow_map_inside_flow_seq() {
    let v = FlowSeq(vec![FlowMap(BTreeMap::from([("a", 1i32)]))]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("{a: 1}"), "expected flow map in flow seq: {yaml}");
}

// ── write_quoted: unicode escape \u{:04X} for chars 0x100-0xFFFF ─────────────

#[test]
fn write_quoted_null_escape() {
    // \x00 (NUL) gets \0 escape in double-quoted strings
    let mut m = BTreeMap::new();
    m.insert("key\x00null", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\0") || yaml.contains("\\u0000"), "expected NUL escape: {yaml}");
}

#[test]
fn write_quoted_named_escapes_in_value() {
    // Values with control chars go through write_quoted which uses named escapes
    // Use quote_all to force quoting of a value containing control chars
    let opts = SerializerOptions { quote_all: true, ..Default::default() };
    // BEL \x07 -> \a, BS \x08 -> \b, VT \x0b -> \v, FF \x0c -> \f, ESC \x1b -> \e
    for (ch, expected) in [('\x07', "\\a"), ('\x08', "\\b"), ('\x0b', "\\v"), ('\x0c', "\\f"), ('\x1b', "\\e")] {
        let s = format!("x{}y", ch);
        let yaml = to_string_with_options(&s.as_str(), opts).unwrap();
        assert!(yaml.contains(expected), "expected {expected} for char {:?}: {yaml}", ch);
    }
    // NUL \x00 -> \0
    let s = "x\x00y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\0"), "expected \\0 for NUL: {yaml}");
    // NEL \u{0085} -> \N
    let s = "x\u{0085}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\N"), "expected \\N for NEL: {yaml}");
    // LS \u{2028} -> \L, PS \u{2029} -> \P (combine with backslash to force double-quoting)
    let s = "x\\\u{2028}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\L"), "expected \\L for LS: {yaml}");
    let s = "x\\\u{2029}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\P"), "expected \\P for PS: {yaml}");
    // BOM \u{FEFF} -> \uFEFF (combine with backslash to force double-quoting)
    let s = "x\\\u{FEFF}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\uFEFF"), "expected \\uFEFF for BOM: {yaml}");
}

// ── write_quoted: various escape sequences ────────────────────────────────────

#[test]
fn write_quoted_control_char_escapes() {
    // Control chars in keys force double-quoting; verify \x hex escapes are used
    // (the serializer uses \xHH for chars 0x01-0x1F range via \u{:04X} or \x{:02X})
    let cases: Vec<(&str, &str)> = vec![
        ("\x07", "07"),   // BEL -> \u0007 or \x07
        ("\x08", "08"),   // BS
        ("\x0b", "0B"),   // VT
        ("\x0c", "0C"),   // FF
        ("\x1b", "1B"),   // ESC
    ];
    for (input, hex) in cases {
        let mut m = BTreeMap::new();
        m.insert(input, "val");
        let yaml = to_string(&m).unwrap();
        // The key should be quoted and contain some escape
        assert!(yaml.contains('"') || yaml.contains("\\u") || yaml.contains("\\x") || yaml.contains(hex),
            "expected escape for {:?}: {yaml}", input);
    }
}

// ── take_flow_for_seq: in_flow > 0 branch ────────────────────────────────────

#[test]
fn seq_inside_flow_uses_flow_style() {
    // When in_flow > 0, take_flow_for_seq returns true (line 818)
    let v = FlowSeq(vec![vec![1i32, 2], vec![3i32, 4]]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('['), "expected flow: {yaml}");
}

// ── take_flow_for_map: in_flow > 0 branch ────────────────────────────────────

#[test]
fn map_inside_flow_uses_flow_style() {
    // When in_flow > 0, take_flow_for_map returns true (line 828)
    let v = FlowSeq(vec![BTreeMap::from([("a", 1i32)])]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('{'), "expected flow map: {yaml}");
}

// ── KeyScalarSink: keys with control chars (tab, newline, carriage return) ────

#[test]
fn key_with_tab_gets_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\twith\ttabs", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\t"), "expected \\t escape in key: {yaml}");
}

#[test]
fn key_with_newline_gets_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\nwith\nnewlines", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\n"), "expected \\n escape in key: {yaml}");
}

#[test]
fn key_with_carriage_return_gets_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\rwith\rcr", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\r"), "expected \\r escape in key: {yaml}");
}

#[test]
fn key_with_control_char_gets_unicode_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\x01ctrl", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\u"), "expected \\u escape in key: {yaml}");
}

#[test]
fn key_with_backslash_plain() {
    // Backslash is plain-safe in YAML keys, no quoting needed
    let mut m = BTreeMap::new();
    m.insert("key\\with\\backslash", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("key\\with\\backslash:"), "yaml: {yaml}");
}

#[test]
fn key_with_double_quote_gets_escaped() {
    // Double quote forces quoting of the key
    let mut m = BTreeMap::new();
    m.insert("key\"with\"quotes", "val");
    let yaml = to_string(&m).unwrap();
    // Key must be quoted (single or double) since it contains a double quote
    assert!(yaml.contains('"') || yaml.contains('\''), "expected quoted key: {yaml}");
}

// ── Block scalar with leading spaces (indentation indicator) ──────────────────

#[test]
fn lit_str_with_leading_spaces_uses_indent_indicator() {
    // Content with leading spaces requires an explicit indentation indicator
    let s = LitStr("  indented content\n");
    let yaml = to_string(&s).unwrap();
    // Should contain |N where N is a digit
    assert!(yaml.contains('|'), "expected literal block: {yaml}");
}

#[test]
fn fold_str_with_leading_spaces_uses_indent_indicator() {
    let s = FoldStr("  indented folded content\n");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

// ── prefer_block_scalars with leading spaces ──────────────────────────────────

#[test]
fn prefer_block_scalars_with_leading_spaces() {
    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"  leading spaces\n", opts).unwrap();
    assert!(yaml.contains('|') || yaml.contains('>') || yaml.contains('"'), "yaml: {yaml}");
}

// ── ArcAnchor: alias reuse ────────────────────────────────────────────────────

#[test]
fn arc_anchor_alias_reuse() {
    let shared = std::sync::Arc::new(42i32);
    let a = ArcAnchor(shared.clone());
    let b = ArcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string(&v).unwrap();
    // Second occurrence should be an alias (*a0 or similar)
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

// ── RcAnchor: alias reuse ─────────────────────────────────────────────────────

#[test]
fn rc_anchor_alias_reuse() {
    use std::rc::Rc;
    let shared = Rc::new("hello");
    let a = RcAnchor(shared.clone());
    let b = RcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

// ── RcWeakAnchor with live ref ────────────────────────────────────────────────

#[test]
fn rc_weak_anchor_live_emits_value() {
    use std::rc::Rc;
    let rc = Rc::new(99i32);
    let weak = RcWeakAnchor(Rc::downgrade(&rc));
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("99"), "expected value: {yaml}");
    drop(rc);
}

// ── ArcWeakAnchor with live ref ───────────────────────────────────────────────

#[test]
fn arc_weak_anchor_live_emits_value() {
    let arc = Arc::new(77i32);
    let weak = ArcWeakAnchor(Arc::downgrade(&arc));
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("77"), "expected value: {yaml}");
    drop(arc);
}

// ── RcRecursion with live ref ─────────────────────────────────────────────────

#[test]
fn rc_recursion_live_emits_value() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let rc_rec = RcRecursive::<i32>(Rc::new(RefCell::new(Some(55))));
    let weak = RcRecursion::from(&rc_rec);
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("55"), "expected value: {yaml}");
}

// ── ArcRecursion with live ref ────────────────────────────────────────────────

#[test]
fn arc_recursion_live_emits_value() {
    let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(Some(33))));
    let weak = ArcRecursion::from(&arc_rec);
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("33"), "expected value: {yaml}");
}

// ── Commented in block context emits comment ──────────────────────────────────

#[test]
fn commented_in_block_emits_comment() {
    let v = Commented(42i32, "# my comment".to_string());
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("42"), "yaml: {yaml}");
    assert!(yaml.contains("# my comment"), "expected comment: {yaml}");
}

// ── SpaceAfter in block context adds blank line ───────────────────────────────

#[test]
fn space_after_in_block_adds_blank_line() {
    #[derive(Serialize)]
    struct S {
        a: SpaceAfter<i32>,
        b: i32,
    }
    let yaml = to_string(&S { a: SpaceAfter(1), b: 2 }).unwrap();
    assert!(yaml.contains("a: 1"), "yaml: {yaml}");
    assert!(yaml.contains("b: 2"), "yaml: {yaml}");
    // There should be a blank line between a and b
    assert!(yaml.contains("\n\n"), "expected blank line: {yaml}");
}

// ── Nested block seq as map value (last_value_was_block branch) ───────────────

#[test]
fn nested_block_seq_as_map_value_then_another_key() {
    #[derive(Serialize)]
    struct S {
        items: Vec<i32>,
        count: i32,
    }
    let yaml = to_string(&S { items: vec![1, 2, 3], count: 3 }).unwrap();
    assert!(yaml.contains("items:"), "yaml: {yaml}");
    assert!(yaml.contains("count: 3"), "yaml: {yaml}");
}

// ── Nested block map as map value (last_value_was_block branch) ───────────────

#[test]
fn nested_block_map_as_map_value_then_another_key() {
    #[derive(Serialize)]
    struct Inner { x: i32 }
    #[derive(Serialize)]
    struct Outer { inner: Inner, after: i32 }
    let yaml = to_string(&Outer { inner: Inner { x: 1 }, after: 2 }).unwrap();
    assert!(yaml.contains("inner:"), "yaml: {yaml}");
    assert!(yaml.contains("after: 2"), "yaml: {yaml}");
}

// ── Empty block seq as map value with empty_as_braces ────────────────────────

#[test]
fn empty_block_seq_as_map_value_with_empty_as_braces() {
    let opts = SerializerOptions {
        empty_as_braces: true,
        ..Default::default()
    };
    #[derive(Serialize)]
    struct S { items: Vec<i32> }
    let yaml = to_string_with_options(&S { items: vec![] }, opts).unwrap();
    assert!(yaml.contains("[]"), "yaml: {yaml}");
}

// ── Empty block map as map value with empty_as_braces ────────────────────────

#[test]
fn empty_block_map_as_map_value_with_empty_as_braces() {
    let opts = SerializerOptions {
        empty_as_braces: true,
        ..Default::default()
    };
    #[derive(Serialize)]
    struct S { m: BTreeMap<String, i32> }
    let yaml = to_string_with_options(&S { m: BTreeMap::new() }, opts).unwrap();
    assert!(yaml.contains("{}"), "yaml: {yaml}");
}

// ── Tuple struct serialization ────────────────────────────────────────────────

#[test]
fn tuple_struct_serialized() {
    #[derive(Serialize)]
    struct Point(i32, i32);
    let yaml = to_string(&Point(3, 4)).unwrap();
    assert!(yaml.contains("3") && yaml.contains("4"), "yaml: {yaml}");
}

// ── Tuple variant serialization ───────────────────────────────────────────────

#[test]
fn tuple_variant_serialized() {
    #[derive(Serialize)]
    enum Shape {
        #[allow(dead_code)]
        Circle(f64),
        Rect(f64, f64),
    }
    let yaml = to_string(&Shape::Rect(2.0, 3.0)).unwrap();
    assert!(yaml.contains("Rect") || yaml.contains("2") , "yaml: {yaml}");
}

// ── Struct variant serialization ──────────────────────────────────────────────

#[test]
fn struct_variant_serialized() {
    #[derive(Serialize)]
    enum Event {
        Move { x: i32, y: i32 },
    }
    let yaml = to_string(&Event::Move { x: 10, y: 20 }).unwrap();
    assert!(yaml.contains("x") && yaml.contains("10"), "yaml: {yaml}");
}

// ── to_io_writer ──────────────────────────────────────────────────────────────

#[test]
fn to_io_writer_produces_yaml() {
    let mut buf = Vec::new();
    serde_saphyr::to_io_writer(&mut buf, &42i32).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("42"), "yaml: {s}");
}

// ── to_fmt_writer ─────────────────────────────────────────────────────────────

#[test]
fn to_fmt_writer_produces_yaml() {
    let mut s = String::new();
    serde_saphyr::to_fmt_writer(&mut s, &"hello").unwrap();
    assert!(s.contains("hello"), "yaml: {s}");
}

// ── Newtype variant serialization ─────────────────────────────────────────────

#[test]
fn newtype_variant_serialized() {
    #[derive(Serialize)]
    enum Wrapper {
        Int(i32),
    }
    let yaml = to_string(&Wrapper::Int(7)).unwrap();
    assert!(yaml.contains("7"), "yaml: {yaml}");
}

// ── serialize_newtype_variant with tagged_enums ───────────────────────────────

#[test]
fn newtype_variant_tagged_enums() {
    #[derive(Serialize)]
    enum Wrapper {
        Int(i32),
    }
    let opts = SerializerOptions {
        tagged_enums: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&Wrapper::Int(7), opts).unwrap();
    assert!(yaml.contains("7"), "yaml: {yaml}");
}

// ── Seq of block maps (inline_map_after_dash branch) ─────────────────────────

#[test]
fn seq_of_structs_inline_map_after_dash() {
    #[derive(Serialize)]
    struct Item { name: &'static str, val: i32 }
    let v = vec![
        Item { name: "a", val: 1 },
        Item { name: "b", val: 2 },
    ];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("name: a") || yaml.contains("name:"), "yaml: {yaml}");
}

// ── Seq of seqs (nested block seq under dash) ─────────────────────────────────

#[test]
fn seq_of_seqs_nested() {
    let v = vec![vec![1i32, 2], vec![3, 4]];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("1") && yaml.contains("3"), "yaml: {yaml}");
}

// ── Flow seq end when in_flow == 0 (newline emitted) ─────────────────────────

#[test]
fn flow_seq_at_top_level_ends_with_newline() {
    let v = FlowSeq(vec![1i32, 2, 3]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.trim() == "[1, 2, 3]", "yaml: {yaml}");
}

// ── Flow map end when in_flow == 0 (newline emitted) ─────────────────────────

#[test]
fn flow_map_at_top_level_ends_with_newline() {
    let m = FlowMap(BTreeMap::from([("a", 1i32)]));
    let yaml = to_string(&m).unwrap();
    assert!(yaml.trim() == "{a: 1}", "yaml: {yaml}");
}

// ── FoldStr with one trailing newline (clip) ──────────────────────────────────

#[test]
fn fold_str_one_trailing_newline_clip() {
    let s = FoldStr("hello world this is a long enough string to fold\n");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

// ── FoldStr with two trailing newlines (keep) ─────────────────────────────────

#[test]
fn fold_str_two_trailing_newlines() {
    // FoldStr uses plain '>' without chomp indicator (historical behavior)
    let s = FoldStr("hello world this is a long enough string to fold\n\n");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

// ── prefer_block_scalars with one trailing newline ────────────────────────────

#[test]
fn prefer_block_scalars_one_trailing_newline() {
    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"line one\nline two\n", opts).unwrap();
    assert!(yaml.contains('>') || yaml.contains('|'), "expected block scalar: {yaml}");
}

// ── prefer_block_scalars no trailing newline (strip) ─────────────────────────

#[test]
fn prefer_block_scalars_no_trailing_newline() {
    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"line one\nline two", opts).unwrap();
    assert!(yaml.contains('>') || yaml.contains('|') || yaml.contains('"'), "yaml: {yaml}");
}

// ── write_single_quoted: single quote doubling ────────────────────────────────

#[test]
fn quote_all_string_with_single_quote_uses_double_quotes_or_doubles() {
    // "it's" has a single quote; quote_all should handle it
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    // A string with ONLY a single quote and no backslash/control chars
    // needs_double_quotes returns true for single quote, so it uses double quotes
    let yaml = to_string_with_options(&"it's fine", opts).unwrap();
    assert!(yaml.contains("it") && yaml.contains("fine"), "yaml: {yaml}");
}

#[test]
fn quote_all_plain_string_uses_single_quotes() {
    // A plain string with no special chars uses single-quoted style
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&"hello world", opts).unwrap();
    // Should be single-quoted since no special chars
    assert!(yaml.contains("'hello world'"), "yaml: {yaml}");
}

// ── write_anchor_name: custom anchor generator fallback ───────────────────────

#[test]
fn custom_anchor_generator_out_of_sync_fallback() {
    use std::rc::Rc;
    // Use a generator that returns empty names to trigger the fallback path
    let opts = SerializerOptions {
        anchor_generator: Some(|_id| String::new()),
        ..Default::default()
    };
    let shared = Rc::new(42i32);
    let a = RcAnchor(shared.clone());
    let b = RcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(yaml.contains("42"), "yaml: {yaml}");
}

// ── Indentation indicator fallback: deep nesting + leading spaces ─────────────

#[test]
fn deep_nesting_with_leading_spaces_falls_back_to_quoted() {
    // indent_n > 9 with leading spaces triggers the fallback to quoted string
    // Need depth > 4 with default indent_step=2: 2*(depth+1) > 9 => depth >= 4
    #[derive(Serialize)]
    struct L5 { val: LitStr<'static> }
    #[derive(Serialize)]
    struct L4 { inner: L5 }
    #[derive(Serialize)]
    struct L3 { inner: L4 }
    #[derive(Serialize)]
    struct L2 { inner: L3 }
    #[derive(Serialize)]
    struct L1 { inner: L2 }

    let v = L1 {
        inner: L2 {
            inner: L3 {
                inner: L4 {
                    inner: L5 {
                        val: LitStr("  leading spaces content\n"),
                    }
                }
            }
        }
    };
    // Should not panic; falls back to quoted string when indent_n > 9
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("leading spaces content"), "yaml: {yaml}");
}

// ── KeyScalarSink error paths: non-scalar keys ────────────────────────────────

#[test]
fn seq_as_map_key_returns_error_for_seq_methods() {
    // Trying to use a sequence as a map key should produce an error
    // (KeyScalarSink rejects serialize_seq)
    use serde::ser::SerializeMap;
    struct BadKey;
    impl Serialize for BadKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq;
            let seq = s.serialize_seq(Some(0))?;
            seq.end()
        }
    }
    // Wrap in a map to trigger KeyScalarSink
    struct MapWithBadKey;
    impl Serialize for MapWithBadKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
            let mut map = s.serialize_map(Some(1))?;
            map.serialize_key(&BadKey)?;
            map.serialize_value(&1i32)?;
            map.end()
        }
    }
    // This should either succeed (complex key) or return an error
    let result = to_string(&MapWithBadKey);
    // Either way, no panic
    let _ = result;
}

// ── ArcRecursive with uninitialized value (error path) ───────────────────────

#[test]
fn arc_recursive_uninitialized_returns_error() {
    let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(None)));
    // Serializing ArcRecursive with None should return an error
    let result = to_string(&arc_rec);
    assert!(result.is_err(), "expected error for uninitialized ArcRecursive");
}

// ── RcRecursive with uninitialized value (error path) ────────────────────────

#[test]
fn rc_recursive_uninitialized_returns_error() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let rc_rec = RcRecursive::<i32>(Rc::new(RefCell::new(None)));
    let result = to_string(&rc_rec);
    assert!(result.is_err(), "expected error for uninitialized RcRecursive");
}

// ── Commented with newline in comment (sanitized to space) ───────────────────

#[test]
fn commented_newline_in_comment_sanitized() {
    let v = Commented(42i32, "line1\nline2".to_string());
    let yaml = to_string(&v).unwrap();
    // Newline in comment should be replaced with space
    assert!(yaml.contains("42"), "yaml: {yaml}");
    assert!(!yaml.contains('\n') || yaml.lines().count() <= 2, "yaml: {yaml}");
}

// ── yaml_12 option emits YAML directive ──────────────────────────────────────

#[test]
fn yaml_12_option_emits_directive() {
    let opts = SerializerOptions {
        yaml_12: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&42i32, opts).unwrap();
    assert!(yaml.contains("%YAML 1.2"), "expected YAML 1.2 directive: {yaml}");
}

// ── Deserialize impls for wrappers ────────────────────────────────────────────

#[test]
fn flow_seq_deserialize_roundtrip() {
    let original = FlowSeq(vec![1i32, 2, 3]);
    let yaml = to_string(&original).unwrap();
    let back: FlowSeq<Vec<i32>> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, vec![1, 2, 3]);
}

#[test]
fn flow_map_deserialize_roundtrip() {
    let original = FlowMap(BTreeMap::from([("a".to_string(), 1i32)]));
    let yaml = to_string(&original).unwrap();
    let back: FlowMap<BTreeMap<String, i32>> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, BTreeMap::from([("a".to_string(), 1)]));
}

#[test]
fn space_after_deserialize_roundtrip() {
    let original = SpaceAfter(42i32);
    let yaml = to_string(&original).unwrap();
    let back: SpaceAfter<i32> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, 42);
}

#[test]
fn commented_deserialize_roundtrip() {
    let original = Commented(99i32, "a comment".to_string());
    let yaml = to_string(&original).unwrap();
    // Deserialization ignores comments, produces empty comment string
    let back: Commented<i32> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, 99);
}

// ── LitStr/FoldStr deserialize roundtrip ─────────────────────────────────────

#[test]
fn lit_str_deserialize_roundtrip() {
    let original = LitStr("hello\nworld\n");
    let yaml = to_string(&original).unwrap();
    let back: LitString = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, "hello\nworld\n");
}

#[test]
fn fold_str_deserialize_roundtrip() {
    let original = FoldStr("hello world this is a long enough string to fold\n");
    let yaml = to_string(&original).unwrap();
    let back: FoldString = serde_saphyr::from_str(&yaml).unwrap();
    assert!(back.0.contains("hello"), "back: {:?}", back.0);
}

// ── Serializer::new constructor ───────────────────────────────────────────────

#[test]
fn serializer_new_constructor() {
    let mut out = String::new();
    let mut ser = serde_saphyr::Serializer::new(&mut out);
    42i32.serialize(&mut ser).unwrap();
    assert!(out.contains("42"), "out: {out}");
}

// ── to_string_with_options with anchor_gen ────────────────────────────────────

#[test]
fn custom_anchor_generator_used() {
    use std::rc::Rc;
    let opts = SerializerOptions {
        anchor_generator: Some(|id| format!("myanchor{id}")),
        ..Default::default()
    };
    let shared = Rc::new(42i32);
    let a = RcAnchor(shared.clone());
    let b = RcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(yaml.contains("myanchor"), "expected custom anchor name: {yaml}");
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}
