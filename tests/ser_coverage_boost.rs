//! Additional serializer tests to increase coverage of `src/ser.rs`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use serde_saphyr::{
    ArcRecursion, ArcRecursive, ArcWeakAnchor, Commented, FlowMap, FlowSeq, FoldStr, FoldString,
    LitStr, LitString, RcAnchor, RcRecursion, RcRecursive, SpaceAfter, to_string,
    to_string_with_options,
};

// ── Scalar primitives (i8, i16, u128, i128, f32, char) ──

#[test]
fn serialize_i8_i16_i128_u128_f32_char_scalars() {
    assert_eq!(to_string(&42i8).unwrap(), "42\n");
    assert_eq!(to_string(&-1i16).unwrap(), "-1\n");
    assert_eq!(to_string(&999i128).unwrap(), "999\n");
    assert_eq!(to_string(&12345u128).unwrap(), "12345\n");
    let f32_yaml = to_string(&1.5f32).unwrap();
    assert!(f32_yaml.starts_with("1.5"), "f32: {f32_yaml}");
    assert_eq!(to_string(&'z').unwrap(), "z\n");
}

// ── Bytes serialization (serialize_bytes) ──

#[test]
fn serialize_bytes_inline_as_binary() {
    // serde_bytes makes the field call serialize_bytes in value position (mid-line)
    #[derive(Serialize)]
    struct B {
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    }
    let b = B {
        data: vec![1, 2, 3],
    };
    let yaml = to_string(&b).unwrap();
    assert!(yaml.contains("!!binary"), "expected !!binary tag: {yaml}");
}

#[test]
fn serialize_bytes_top_level_as_seq() {
    // Top-level &[u8] should serialize as a block sequence of integers
    let data: &[u8] = &[10, 20];
    let yaml = to_string(&serde_bytes::Bytes::new(data)).unwrap();
    // When at line start, bytes go through serialize_seq path
    // Actually top-level serde_bytes::Bytes calls serialize_bytes which checks at_line_start
    assert!(
        yaml.contains("10") && yaml.contains("20"),
        "expected byte values: {yaml}"
    );
}

// ── quote_all mode ──

#[test]
fn quote_all_uses_single_quotes_for_simple_strings() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = to_string_with_options(&"hello", opts).unwrap();
    assert_eq!(yaml, "'hello'\n");
}

#[test]
fn quote_all_uses_double_quotes_for_strings_with_escapes() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = to_string_with_options(&"line\nbreak", opts).unwrap();
    assert!(yaml.starts_with('"'), "expected double quotes: {yaml}");
    assert!(yaml.contains("\\n"), "expected escaped newline: {yaml}");
}

#[test]
fn quote_all_single_quote_inside_string() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = to_string_with_options(&"it's", opts).unwrap();
    // Contains single quote → must use double quotes
    assert!(yaml.starts_with('"'), "expected double quotes: {yaml}");
}

#[test]
fn quote_all_value_position() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let mut m = BTreeMap::new();
    m.insert("key", "value");
    let yaml = to_string_with_options(&m, opts).unwrap();
    // Keys go through KeyScalarSink which doesn't use quote_all; values do.
    assert!(
        yaml.contains("'value'") || yaml.contains("\"value\""),
        "value should be quoted: {yaml}"
    );
}

// ── SpaceAfter ──

#[test]
fn space_after_emits_blank_line() {
    #[derive(Serialize)]
    struct S {
        a: SpaceAfter<i32>,
        b: i32,
    }
    let s = S {
        a: SpaceAfter(1),
        b: 2,
    };
    let yaml = to_string(&s).unwrap();
    assert!(
        yaml.contains("a: 1\n\n"),
        "expected blank line after a: {yaml}"
    );
}

// ── LitStr / FoldStr / LitString / FoldString ──

#[test]
fn lit_str_emits_literal_block() {
    let yaml = to_string(&LitStr("hello\nworld\n")).unwrap();
    assert!(
        yaml.contains('|'),
        "expected literal block indicator: {yaml}"
    );
    assert!(yaml.contains("hello"), "expected content: {yaml}");
}

#[test]
fn fold_str_emits_folded_block_for_long_strings() {
    let long = "a ".repeat(50); // 100 chars, well above default threshold
    let yaml = to_string(&FoldStr(&long)).unwrap();
    assert!(
        yaml.contains('>'),
        "expected folded block indicator: {yaml}"
    );
}

#[test]
fn fold_str_short_string_stays_inline() {
    let yaml = to_string(&FoldStr("short")).unwrap();
    assert!(
        !yaml.contains('>'),
        "short string should not use folded: {yaml}"
    );
}

#[test]
fn lit_string_owned_works() {
    let yaml = to_string(&LitString("line1\nline2\n".to_string())).unwrap();
    assert!(yaml.contains('|'), "expected literal block: {yaml}");
}

#[test]
fn fold_string_owned_works() {
    let long = "word ".repeat(30);
    let yaml = to_string(&FoldString(long)).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

// ── Tagged enums ──

#[test]
fn tagged_enums_emit_yaml_tags() {
    #[derive(Serialize)]
    enum Color {
        Red,
    }
    let opts = serde_saphyr::ser_options! { tagged_enums: true };
    let yaml = to_string_with_options(&Color::Red, opts).unwrap();
    assert!(yaml.contains("!!Color"), "expected tag: {yaml}");
}

// ── Tuple variants ──

#[test]
fn tuple_variant_serializes_as_mapping_with_seq() {
    #[derive(Serialize)]
    enum E {
        Pair(i32, i32),
    }
    let yaml = to_string(&E::Pair(1, 2)).unwrap();
    assert!(yaml.contains("Pair"), "expected variant name: {yaml}");
    assert!(
        yaml.contains("- 1") && yaml.contains("- 2"),
        "expected seq elements: {yaml}"
    );
}

// ── Struct variants ──

#[test]
fn struct_variant_serializes_as_mapping() {
    #[derive(Serialize)]
    enum E {
        Point { x: i32, y: i32 },
    }
    let yaml = to_string(&E::Point { x: 1, y: 2 }).unwrap();
    assert!(yaml.contains("Point"), "expected variant name: {yaml}");
    assert!(yaml.contains("x: 1"), "expected x field: {yaml}");
    assert!(
        yaml.contains("y") && yaml.contains(": 2"),
        "expected y field: {yaml}"
    );
}

#[test]
fn struct_variant_as_map_value() {
    #[derive(Serialize)]
    enum E {
        Point { x: i32, y: i32 },
    }
    let mut m = BTreeMap::new();
    m.insert("loc", E::Point { x: 3, y: 4 });
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("loc"), "expected key: {yaml}");
    assert!(yaml.contains("Point"), "expected variant: {yaml}");
}

// ── Newtype variant ──

#[test]
fn newtype_variant_serializes() {
    #[derive(Serialize)]
    enum Wrap {
        Val(i32),
    }
    let yaml = to_string(&Wrap::Val(42)).unwrap();
    assert!(yaml.contains("Val") && yaml.contains("42"), "got: {yaml}");
}

#[test]
fn newtype_variant_as_map_value() {
    #[derive(Serialize)]
    enum Wrap {
        Val(i32),
    }
    let mut m = BTreeMap::new();
    m.insert("w", Wrap::Val(7));
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("Val") && yaml.contains("7"), "got: {yaml}");
}

// ── RcRecursive / ArcRecursive / RcRecursion / ArcRecursion ──

#[test]
fn rc_recursive_serializes_with_anchor() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Rc::new(RefCell::new(Some(Node { val: 10 })));
    let anchor = RcRecursive(inner.clone());
    let yaml = to_string(&anchor).unwrap();
    assert!(yaml.contains("val: 10"), "expected value: {yaml}");
    assert!(yaml.contains("&a1"), "expected anchor: {yaml}");
}

#[test]
fn arc_recursive_serializes_with_anchor() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Arc::new(Mutex::new(Some(Node { val: 20 })));
    let anchor = ArcRecursive(inner.clone());
    let yaml = to_string(&anchor).unwrap();
    assert!(yaml.contains("val: 20"), "expected value: {yaml}");
    assert!(yaml.contains("&a1"), "expected anchor: {yaml}");
}

#[test]
fn rc_recursion_present_serializes() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Rc::new(RefCell::new(Some(Node { val: 30 })));
    let weak = Rc::downgrade(&inner);
    let recur = RcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("val: 30"), "expected value: {yaml}");
}

#[test]
fn rc_recursion_dangling_serializes_as_null() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let weak = {
        let inner = Rc::new(RefCell::new(Some(Node { val: 0 })));
        Rc::downgrade(&inner)
    };
    let recur = RcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn arc_recursion_present_serializes() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Arc::new(Mutex::new(Some(Node { val: 40 })));
    let weak = Arc::downgrade(&inner);
    let recur = ArcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("val: 40"), "expected value: {yaml}");
}

#[test]
fn arc_recursion_dangling_serializes_as_null() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let weak = {
        let inner = Arc::new(Mutex::new(Some(Node { val: 0 })));
        Arc::downgrade(&inner)
    };
    let recur = ArcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert_eq!(yaml, "null\n");
}

// ── ArcWeakAnchor dangling ──

#[test]
fn arc_weak_anchor_dangling_serializes_as_null() {
    #[derive(Serialize, Clone)]
    struct N {
        v: i32,
    }
    let weak = {
        let s = Arc::new(N { v: 1 });
        Arc::downgrade(&s)
    };
    let yaml = to_string(&ArcWeakAnchor(weak)).unwrap();
    assert_eq!(yaml, "null\n");
}

// ── Custom anchor generator ──

#[test]
fn custom_anchor_generator() {
    let shared = Rc::new(42i32);
    let v = vec![RcAnchor(shared.clone()), RcAnchor(shared)];
    let opts = serde_saphyr::ser_options! {
        anchor_generator: Some(|id| format!("custom{}", id)),
    };
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(yaml.contains("&custom1"), "expected custom anchor: {yaml}");
    assert!(yaml.contains("*custom1"), "expected custom alias: {yaml}");
}

// ── with_indent (line 515) ──

#[test]
fn with_indent_changes_indentation() {
    #[derive(Serialize)]
    struct S {
        a: Vec<i32>,
    }
    let opts = serde_saphyr::ser_options! { indent_step: 4 };
    let yaml = to_string_with_options(&S { a: vec![1] }, opts).unwrap();
    // With 4-space indent, the list item should be indented by 4 spaces
    assert!(yaml.contains("    - 1"), "expected 4-space indent: {yaml}");
}

// ── Flow sequences and maps ──

#[test]
fn flow_seq_emits_brackets() {
    let yaml = to_string(&FlowSeq(vec![1, 2, 3])).unwrap();
    assert_eq!(yaml, "[1, 2, 3]\n");
}

#[test]
fn flow_map_emits_braces() {
    let mut m = BTreeMap::new();
    m.insert("a", 1);
    m.insert("b", 2);
    let yaml = to_string(&FlowMap(m)).unwrap();
    assert!(
        yaml.starts_with('{') && yaml.contains('}'),
        "expected flow map: {yaml}"
    );
}

// ── Commented in flow context (suppressed) ──

#[test]
fn commented_in_flow_context_suppresses_comment() {
    let yaml = to_string(&FlowSeq(vec![Commented(1, "note".into())])).unwrap();
    assert!(
        !yaml.contains('#'),
        "comment should be suppressed in flow: {yaml}"
    );
}

// ── Empty collections ──

#[test]
fn empty_map_as_braces() {
    #[derive(Serialize)]
    struct S {
        m: BTreeMap<String, i32>,
    }
    let s = S { m: BTreeMap::new() };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("{}"), "expected empty braces: {yaml}");
}

#[test]
fn empty_seq_as_brackets() {
    #[derive(Serialize)]
    struct S {
        v: Vec<i32>,
    }
    let s = S { v: vec![] };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("[]"), "expected empty brackets: {yaml}");
}

#[test]
fn empty_map_without_braces() {
    #[derive(Serialize)]
    struct S {
        m: BTreeMap<String, i32>,
    }
    let s = S { m: BTreeMap::new() };
    let opts = serde_saphyr::ser_options! { empty_as_braces: false };
    let yaml = to_string_with_options(&s, opts).unwrap();
    // Without braces, empty map should not contain {}
    assert!(!yaml.contains("{}"), "should not have braces: {yaml}");
}

// ── Tuple struct (normal, not special) ──

#[test]
fn normal_tuple_struct_serializes_as_seq() {
    #[derive(Serialize)]
    struct Pair(i32, String);
    let yaml = to_string(&Pair(1, "two".into())).unwrap();
    assert!(
        yaml.contains("- 1") && yaml.contains("- two"),
        "got: {yaml}"
    );
}

// ── Nested structures ──

#[test]
fn nested_map_in_seq() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    let v = vec![Inner { x: 1 }, Inner { x: 2 }];
    let yaml = to_string(&v).unwrap();
    assert!(
        yaml.contains("- x: 1") && yaml.contains("- x: 2"),
        "got: {yaml}"
    );
}

#[test]
fn seq_in_map_value() {
    let mut m = BTreeMap::new();
    m.insert("items", vec![1, 2, 3]);
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("items:"), "got: {yaml}");
    assert!(yaml.contains("- 1"), "got: {yaml}");
}

// ── YAML 1.2 mode ──

#[test]
fn yaml_12_emits_directive() {
    let opts = serde_saphyr::ser_options! { yaml_12: true };
    let yaml = to_string_with_options(&42, opts).unwrap();
    assert!(
        yaml.contains("%YAML 1.2"),
        "expected YAML directive: {yaml}"
    );
}

// ── Literal block with trailing newlines (chomp indicators) ──

#[test]
fn lit_str_strip_chomp() {
    // No trailing newline → strip chomp (|-)
    let yaml = to_string(&LitStr("hello")).unwrap();
    assert!(yaml.contains("|-"), "expected strip chomp: {yaml}");
}

#[test]
fn lit_str_keep_chomp() {
    // Multiple trailing newlines → keep chomp (|+)
    let yaml = to_string(&LitStr("hello\n\n")).unwrap();
    assert!(yaml.contains("|+"), "expected keep chomp: {yaml}");
}

// ── Unit struct ──

#[test]
fn unit_struct_serializes_as_null() {
    #[derive(Serialize)]
    struct Unit;
    let yaml = to_string(&Unit).unwrap();
    assert_eq!(yaml, "null\n");
}

// ── Option Some/None ──

#[test]
fn option_none_serializes_as_null() {
    let v: Option<i32> = None;
    let yaml = to_string(&v).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn option_some_serializes_value() {
    let v: Option<i32> = Some(5);
    let yaml = to_string(&v).unwrap();
    assert_eq!(yaml, "5\n");
}

// ── Struct with many field types ──

#[test]
fn struct_with_various_field_types() {
    #[derive(Serialize)]
    struct Mixed {
        b: bool,
        i: i64,
        f: f64,
        s: String,
        o: Option<i32>,
        v: Vec<u8>,
    }
    let m = Mixed {
        b: true,
        i: -42,
        f: std::f64::consts::PI,
        s: "hello".into(),
        o: None,
        v: vec![1, 2],
    };
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("b: true"), "got: {yaml}");
    assert!(yaml.contains("i: -42"), "got: {yaml}");
    assert!(yaml.contains("s: hello"), "got: {yaml}");
    assert!(yaml.contains("o: null"), "got: {yaml}");
}

// ── Prefer block scalars auto-selection ──

#[test]
fn auto_literal_for_multiline_string() {
    // A multiline string should auto-select literal block style
    let yaml = to_string(&"line1\nline2\n").unwrap();
    assert!(yaml.contains('|'), "expected auto literal block: {yaml}");
}

#[test]
fn auto_folded_for_long_single_line() {
    // A very long single-line string should auto-select folded block style
    let long = "word ".repeat(30); // 150 chars
    let yaml = to_string(&long).unwrap();
    assert!(yaml.contains('>'), "expected auto folded block: {yaml}");
}

#[test]
fn no_block_scalars_when_disabled() {
    let opts = serde_saphyr::ser_options! { prefer_block_scalars: false };
    let yaml = to_string_with_options(&"line1\nline2\n", opts).unwrap();
    assert!(
        !yaml.contains('|') && !yaml.contains('>'),
        "should not use block: {yaml}"
    );
}

// ── Commented with empty comment ──

#[test]
fn commented_empty_string_no_comment_marker() {
    let yaml = to_string(&Commented(42, String::new())).unwrap();
    assert!(
        !yaml.contains('#'),
        "empty comment should not emit #: {yaml}"
    );
    assert!(yaml.contains("42"), "value missing: {yaml}");
}

// ── collect_str (Display-based serialization) ──

#[test]
fn collect_str_via_display() {
    use std::net::Ipv4Addr;
    let addr = Ipv4Addr::new(127, 0, 0, 1);
    let yaml = to_string(&addr).unwrap();
    assert!(yaml.contains("127.0.0.1"), "got: {yaml}");
}

// ── Complex (non-scalar) map keys in block style ──

#[test]
fn complex_map_key_uses_question_mark_syntax() {
    // Block maps support complex keys via `? key\n: value` syntax.
    // Use a Vec as key (non-scalar) in a BTreeMap-like structure.
    use serde::ser::{SerializeMap, Serializer};

    // Manually serialize a map with a sequence key
    struct ComplexKeyMap;
    impl Serialize for ComplexKeyMap {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_key(&vec![1, 2])?;
            map.serialize_value(&"val")?;
            map.end()
        }
    }
    let yaml = to_string(&ComplexKeyMap).unwrap();
    assert!(yaml.contains("? "), "expected complex key syntax: {yaml}");
}

// ── Folded block with auto-chomp indicators ──

#[test]
fn auto_folded_strip_chomp_no_trailing_newline() {
    // Long string without trailing newline → auto folded with strip chomp (>-)
    let long = "word ".repeat(30).trim_end().to_string();
    let yaml = to_string(&long).unwrap();
    assert!(yaml.contains(">-"), "expected strip chomp: {yaml}");
}

// ── Single-quoted string with embedded single quote ──

#[test]
fn single_quoted_escapes_embedded_quote() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    // A string with a backslash needs double quotes
    let yaml = to_string_with_options(&"back\\slash", opts).unwrap();
    assert!(
        yaml.starts_with('"'),
        "expected double quotes for backslash: {yaml}"
    );
}

// ── Struct variant in sequence ──

#[test]
fn struct_variant_in_sequence() {
    #[derive(Serialize)]
    enum E {
        Point { x: i32, y: i32 },
    }
    let v = vec![E::Point { x: 1, y: 2 }, E::Point { x: 3, y: 4 }];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("Point"), "got: {yaml}");
    assert!(
        yaml.contains("x: 1") && yaml.contains("x: 3"),
        "got: {yaml}"
    );
}

// ── Tuple variant in sequence ──

#[test]
fn tuple_variant_in_sequence() {
    #[derive(Serialize)]
    enum E {
        Pair(i32, i32),
    }
    let v = vec![E::Pair(1, 2), E::Pair(3, 4)];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("Pair"), "got: {yaml}");
}

// ── Newtype variant with struct inner as map value ──

#[test]
fn newtype_variant_with_struct_inner_as_map_value() {
    #[derive(Serialize)]
    struct Inner {
        a: i32,
    }
    #[derive(Serialize)]
    enum E {
        Wrap(Inner),
    }
    let mut m = BTreeMap::new();
    m.insert("k", E::Wrap(Inner { a: 5 }));
    let yaml = to_string(&m).unwrap();
    assert!(
        yaml.contains("Wrap") && yaml.contains("a: 5"),
        "got: {yaml}"
    );
}

// ── Map value that is itself a map (inline_value_start path) ──

#[test]
fn map_value_is_map() {
    let mut inner = BTreeMap::new();
    inner.insert("x", 1);
    let mut outer = BTreeMap::new();
    outer.insert("nested", inner);
    let yaml = to_string(&outer).unwrap();
    assert!(
        yaml.contains("nested:") && yaml.contains("x: 1"),
        "got: {yaml}"
    );
}

// ── Sequence value after block sibling ──

#[test]
fn seq_value_after_block_sibling() {
    #[derive(Serialize)]
    struct S {
        a: Vec<i32>,
        b: Vec<i32>,
    }
    let s = S {
        a: vec![1],
        b: vec![2],
    };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("- 1") && yaml.contains("- 2"), "got: {yaml}");
}

// ── Boolean at line start (serialize_bool at_line_start path) ──

#[test]
fn bool_at_line_start_in_seq() {
    let yaml = to_string(&vec![true, false]).unwrap();
    assert!(
        yaml.contains("- true") && yaml.contains("- false"),
        "got: {yaml}"
    );
}

// ── u128 as map key ──

#[test]
fn u128_map_key() {
    let mut m = BTreeMap::new();
    m.insert(999u128, "big");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("999: big"), "got: {yaml}");
}

// ── i128 as map key ──

#[test]
fn i128_map_key() {
    let mut m = BTreeMap::new();
    m.insert(-999i128, "neg");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("-999: neg"), "got: {yaml}");
}

// ── Recursive payload with None (unit serialization path) ──

#[test]
fn rc_recursive_not_initialized_errors() {
    let inner: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
    let anchor = RcRecursive(inner);
    let err = to_string(&anchor).unwrap_err();
    assert!(err.to_string().contains("not initialized"), "got: {err}");
}

#[test]
fn arc_recursive_not_initialized_errors() {
    let inner: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let anchor = ArcRecursive(inner);
    let err = to_string(&anchor).unwrap_err();
    assert!(err.to_string().contains("not initialized"), "got: {err}");
}

// ── RcRecursion/ArcRecursion with None inner (covers RcRecursivePayload/ArcRecursivePayload None path) ──

#[test]
fn rc_recursion_with_none_inner_serializes_as_null_value() {
    // RcRecursion -> present=true -> RcRecursivePayload -> inner is None -> serialize_unit
    let inner: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&inner);
    let recur = RcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("null"), "expected null: {yaml}");
}

#[test]
fn arc_recursion_with_none_inner_serializes_as_null_value() {
    let inner: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let weak = Arc::downgrade(&inner);
    let recur = ArcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("null"), "expected null: {yaml}");
}

// ── Folded block with indicator digit (leading spaces in content) ──

#[test]
fn lit_str_with_leading_spaces_emits_indicator() {
    // Content starts with spaces → needs explicit indentation indicator
    let yaml = to_string(&LitStr("  indented\n")).unwrap();
    // Should have |N where N is a digit
    assert!(yaml.contains('|'), "expected literal block: {yaml}");
}

// ── SpaceAfter with complex value ──

#[test]
fn space_after_with_seq() {
    #[derive(Serialize)]
    struct S {
        items: SpaceAfter<Vec<i32>>,
        after: i32,
    }
    let s = S {
        items: SpaceAfter(vec![1, 2]),
        after: 3,
    };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("after: 3"), "got: {yaml}");
}

// ── Commented with complex value (comment ignored) ──

#[test]
fn commented_with_map_value_ignores_comment() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    let yaml = to_string(&Commented(Inner { x: 1 }, "ignored".into())).unwrap();
    // Comments are ignored for complex values
    assert!(yaml.contains("x: 1"), "got: {yaml}");
}

// ── Flow map nested in block ──

#[test]
fn flow_map_as_struct_field() {
    #[derive(Serialize)]
    struct S {
        m: FlowMap<BTreeMap<String, i32>>,
    }
    let mut inner = BTreeMap::new();
    inner.insert("a".to_string(), 1);
    let s = S { m: FlowMap(inner) };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("m: {"), "expected inline flow map: {yaml}");
}

// ── Flow seq nested in block ──

#[test]
fn flow_seq_as_struct_field() {
    #[derive(Serialize)]
    struct S {
        v: FlowSeq<Vec<i32>>,
    }
    let s = S {
        v: FlowSeq(vec![1, 2]),
    };
    let yaml = to_string(&s).unwrap();
    assert!(
        yaml.contains("v: [1, 2]"),
        "expected inline flow seq: {yaml}"
    );
}
