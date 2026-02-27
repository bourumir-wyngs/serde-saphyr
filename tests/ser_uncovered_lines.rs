//! Tests targeting previously uncovered lines in src/ser.rs.

use serde::Serialize;
use serde_saphyr::{RcAnchor, RcWeakAnchor, SerializerOptions, to_string};
use std::rc::Rc;

// ~725: Control characters in double-quoted strings trigger \x{:02X} escape.
// Characters like \x7F (DEL) are in the 0x7F..=0x9F range and <= 0xFF,
// so they hit the `\x` escape branch.
#[test]
fn control_char_in_string_uses_hex_escape() {
    // \x7F = DEL, \x80 = padding char, \x9F = last C1 control
    let s = "hello\x7Fworld";
    let yaml = to_string(&s).unwrap();
    assert!(
        yaml.contains("\\x7F"),
        "Expected \\x7F escape, got: {}",
        yaml
    );

    let s2 = "a\u{0080}b";
    let yaml2 = to_string(&s2).unwrap();
    assert!(
        yaml2.contains("\\x80"),
        "Expected \\x80 escape, got: {}",
        yaml2
    );

    let s3 = "x\u{009F}y";
    let yaml3 = to_string(&s3).unwrap();
    assert!(
        yaml3.contains("\\x9F"),
        "Expected \\x9F escape, got: {}",
        yaml3
    );
}

// ~2061–2067, ~2142–2196: Struct variant with nested sequence and map values
// exercises pending_space_after_colon and indentation in StructVariantSer.
#[derive(Serialize)]
enum Outer {
    Variant {
        items: Vec<i32>,
        meta: std::collections::BTreeMap<String, String>,
        flag: bool,
    },
}

#[test]
fn struct_variant_with_seq_and_map() {
    let mut meta = std::collections::BTreeMap::new();
    meta.insert("k1".into(), "v1".into());
    meta.insert("k2".into(), "v2".into());

    let val = Outer::Variant {
        items: vec![1, 2, 3],
        meta,
        flag: true,
    };
    let yaml = to_string(&val).unwrap();
    // Should produce valid YAML with the variant name, then indented fields
    assert!(yaml.contains("Variant"), "Missing variant name: {}", yaml);
    assert!(yaml.contains("items:"), "Missing items field: {}", yaml);
    assert!(yaml.contains("- 1"), "Missing seq element: {}", yaml);
    assert!(yaml.contains("meta:"), "Missing meta field: {}", yaml);
    assert!(yaml.contains("k1: v1"), "Missing map entry: {}", yaml);
    assert!(yaml.contains("flag: true"), "Missing flag field: {}", yaml);
}

// Struct variant with only scalar values (exercises pending_space_after_colon path)
#[derive(Serialize)]
enum Simple {
    Data { x: i32, y: String },
}

#[test]
fn struct_variant_scalar_fields() {
    let val = Simple::Data {
        x: 42,
        y: "hello".into(),
    };
    let yaml = to_string(&val).unwrap();
    assert!(yaml.contains("Data"), "Missing variant: {}", yaml);
    assert!(yaml.contains("x: 42"), "Missing x field: {}", yaml);
    assert!(yaml.contains("hello"), "Missing y/hello field: {}", yaml);
}

// ~803: write_alias_id when at_line_start (alias as sequence element)
// The alias appears as a sequence element, which means at_line_start is true.
#[test]
fn alias_at_line_start_in_sequence() {
    #[derive(Clone, Serialize)]
    struct Leaf {
        val: i32,
    }
    let shared = Rc::new(Leaf { val: 99 });
    let items = vec![
        RcAnchor(shared.clone()),
        RcAnchor(shared.clone()),
        RcAnchor(shared),
    ];
    let yaml = to_string(&items).unwrap();
    // First occurrence defines anchor, subsequent ones are aliases
    assert!(yaml.contains("&a1"), "Missing anchor: {}", yaml);
    assert!(yaml.contains("*a1"), "Missing alias: {}", yaml);
}

// ~867: serialize_bool at line start (bool as top-level sequence element)
#[test]
fn bool_sequence_elements_at_line_start() {
    let bools = vec![true, false, true];
    let yaml = to_string(&bools).unwrap();
    assert_eq!(yaml, "- true\n- false\n- true\n");
}

// Weak anchor alias (exercises write_alias_id for weak refs)
#[test]
fn weak_anchor_alias_in_struct() {
    #[derive(Clone, Serialize)]
    struct Node {
        name: String,
        child: Option<RcAnchor<Node>>,
        back: Option<RcWeakAnchor<Node>>,
    }
    let parent = Rc::new(Node {
        name: "parent".into(),
        child: None,
        back: None,
    });
    let child_node = Node {
        name: "child".into(),
        child: None,
        back: Some(RcWeakAnchor(Rc::downgrade(&parent))),
    };
    let root = Node {
        name: "root".into(),
        child: Some(RcAnchor(Rc::new(child_node))),
        back: Some(RcWeakAnchor(Rc::downgrade(&parent))),
    };
    // The parent Rc is referenced via weak from two places but never via strong anchor,
    // so weak refs should serialize as null (not upgraded).
    let yaml = to_string(&root).unwrap();
    assert!(yaml.contains("name: root"), "Missing root name: {}", yaml);
}

// Struct variant with nested struct (deeper nesting for indentation coverage)
#[derive(Serialize)]
enum Deep {
    Level { inner: Inner, list: Vec<Inner> },
}

#[derive(Serialize)]
struct Inner {
    a: i32,
    b: i32,
}

#[test]
fn struct_variant_nested_struct_and_seq_of_structs() {
    let val = Deep::Level {
        inner: Inner { a: 1, b: 2 },
        list: vec![Inner { a: 3, b: 4 }, Inner { a: 5, b: 6 }],
    };
    let yaml = to_string(&val).unwrap();
    assert!(yaml.contains("Level"), "Missing variant: {}", yaml);
    assert!(yaml.contains("inner:"), "Missing inner: {}", yaml);
    assert!(yaml.contains("a: 1"), "Missing a:1: {}", yaml);
    assert!(yaml.contains("list:"), "Missing list: {}", yaml);
    assert!(yaml.contains("a: 3"), "Missing a:3: {}", yaml);
}

// ~867: Top-level bool serialization (at_line_start is true initially)
#[test]
fn top_level_bool_at_line_start() {
    let yaml = to_string(&true).unwrap();
    assert_eq!(yaml, "true\n");
    let yaml2 = to_string(&false).unwrap();
    assert_eq!(yaml2, "false\n");
}

// ~2061-2067: inline_value_start path in MapSer::serialize_key
// Triggered when a map is serialized as a value of another map key,
// with empty_as_braces enabled and len unknown (HashMap).
// The first field must be a HashMap so last_value_was_block is false (no forced_newline).
#[test]
fn inline_value_start_map_as_value() {
    use std::collections::HashMap;
    // HashMap<String, HashMap<String, i32>> — inner HashMap has _len=None
    // and appears as a value of an outer map key (was_inline_value=true).
    let mut inner = HashMap::new();
    inner.insert("x".into(), 1);
    let mut outer: HashMap<String, HashMap<String, i32>> = HashMap::new();
    outer.insert("key".into(), inner);
    let opts = SerializerOptions::default();
    let yaml = serde_saphyr::to_string_with_options(&outer, opts).unwrap();
    assert!(yaml.contains("key:"), "Missing outer key: {}", yaml);
    assert!(yaml.contains("x: 1"), "Missing inner entry: {}", yaml);
}

// Control char \x01 (SOH) — triggers named escape path (already covered),
// but \x10 (DLE) has no named escape and hits the \x escape branch
#[test]
fn control_char_dle_in_string() {
    let s = "a\x10b";
    let yaml = to_string(&s).unwrap();
    assert!(
        yaml.contains("\\x10"),
        "Expected \\x10 escape, got: {}",
        yaml
    );
}

// Map key with control character — exercises KeyScalarSink \u escape for keys
#[test]
fn map_key_with_control_char() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert("a\x01b".to_string(), 1);
    let yaml = to_string(&m).unwrap();
    // Key should be double-quoted with escape
    assert!(
        yaml.contains("\\x01") || yaml.contains("\\u0001"),
        "Expected escape in key: {}",
        yaml
    );
}

// ~725: The \u{:04X} branch in write_quoted is dead code.
// It requires a char in 0x100..=0xFFFF where is_control() is true,
// but Unicode has no Cc (control) characters above U+009F.
// C1 controls (0x7F-0x9F) are all <= 0xFF and hit the \x{:02X} branch first.
// This test documents that C1 controls use \x escapes (or named escapes).
#[test]
fn c1_control_char_uses_hex_escape() {
    // \u{0085} = NEL has named escape \N
    let s = "a\u{0085}b";
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("\\N"), "Expected \\N escape, got: {}", yaml);
    // \u{008A} has no named escape, hits \x branch
    let s2 = "a\u{008A}b";
    let yaml2 = to_string(&s2).unwrap();
    assert!(
        yaml2.contains("\\x8A"),
        "Expected \\x8A escape, got: {}",
        yaml2
    );
}

// ~556: Single-quote escape in write_single_quoted is dead code.
// The only callers guard with !needs_double_quotes(s), which returns true
// when the string contains '\'', so write_single_quoted never receives a
// string with single quotes. This test documents that quote_all mode with
// a single-quote character correctly falls through to double-quoting.
#[test]
fn quote_all_with_single_quote_uses_double_quotes() {
    let opts = SerializerOptions {
        quote_all: true,
        ..Default::default()
    };
    let yaml = serde_saphyr::to_string_with_options(&"it's", opts).unwrap();
    // Should be double-quoted because single-quote triggers needs_double_quotes
    assert!(
        yaml.contains("\"it's\""),
        "Expected double-quoted: {}",
        yaml
    );
}

// ~2061-2067: struct variant with a map field value exercises pending_space_after_colon
// in the StructVariantSer path
#[test]
fn struct_variant_with_map_field() {
    use std::collections::BTreeMap;
    #[derive(Serialize)]
    enum E {
        V { data: BTreeMap<String, i32> },
    }
    let mut m = BTreeMap::new();
    m.insert("x".into(), 1);
    m.insert("y".into(), 2);
    let val = E::V { data: m };
    let yaml = to_string(&val).unwrap();
    assert!(yaml.contains("V:"), "Missing variant: {}", yaml);
    assert!(yaml.contains("data:"), "Missing data field: {}", yaml);
    assert!(yaml.contains("x: 1"), "Missing x: {}", yaml);
}

// ~2142-2196: Sequence inside a struct variant field
#[test]
fn struct_variant_with_seq_field() {
    #[derive(Serialize)]
    enum E {
        V { items: Vec<i32> },
    }
    let val = E::V {
        items: vec![10, 20, 30],
    };
    let yaml = to_string(&val).unwrap();
    assert!(yaml.contains("V:"), "Missing variant: {}", yaml);
    assert!(yaml.contains("items:"), "Missing items: {}", yaml);
    assert!(yaml.contains("- 10"), "Missing 10: {}", yaml);
}
