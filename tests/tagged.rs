#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_saphyr::{
    Commented, FlowMap, FlowSeq, RcAnchor, SerializeError, Tagged, from_str, ser_options,
    to_string, to_string_with_options,
};

#[test]
fn resolved_core_verbatim_and_directive_tags_normalize_on_output() {
    let core: Tagged<i32> = from_str("!!int 42").unwrap();
    assert_eq!(core, Tagged(42, Some("tag:yaml.org,2002:int".into())));
    let yaml = to_string(&core).unwrap();
    assert_eq!(yaml, "!<tag:yaml.org,2002:int> 42\n");
    assert_eq!(from_str::<Tagged<i32>>(&yaml).unwrap(), core);

    let verbatim: Tagged<String> = from_str("!<tag:example.com,2026:widget> value").unwrap();
    assert_eq!(verbatim.1.as_deref(), Some("tag:example.com,2026:widget"));
    let yaml = to_string(&verbatim).unwrap();
    assert_eq!(yaml, "!<tag:example.com,2026:widget> value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), verbatim);

    let directive: Tagged<String> =
        from_str("%TAG !e! tag:example.com,2026:\n--- !e!widget value\n").unwrap();
    assert_eq!(directive.1.as_deref(), Some("tag:example.com,2026:widget"));
    let yaml = to_string(&directive).unwrap();
    assert_eq!(yaml, "!<tag:example.com,2026:widget> value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), directive);
}

#[test]
fn local_and_global_resolved_tags_are_safely_percent_encoded() {
    let local = Tagged("value".to_owned(), Some("!space here/%".to_owned()));
    let yaml = to_string(&local).unwrap();
    assert_eq!(yaml, "!space%20here/%25 value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), local);

    let global = Tagged(
        "value".to_owned(),
        Some("tag:example.com,2026:a>b % snowman ☃".to_owned()),
    );
    let yaml = to_string(&global).unwrap();
    assert_eq!(
        yaml,
        "!<tag:example.com,2026:a%3Eb%20%25%20snowman%20%E2%98%83> value\n"
    );
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), global);
}

#[test]
fn encoded_global_tag_delimiters_round_trip_contextually() {
    for (input, expected) in [
        (
            "!<http://example.com/path%5B0%5D> value\n",
            "!<http://example.com/path%5B0%5D> value\n",
        ),
        (
            "!<http://example%5B0%5D.com/path> value\n",
            "!<http://example%5B0%5D.com/path> value\n",
        ),
        (
            "!<http://example.com/path?q=%5B0%5D#%5B1%5D> value\n",
            "!<http://example.com/path?q=%5B0%5D#%5B1%5D> value\n",
        ),
        (
            "!<http://user%40name@example.com/> value\n",
            "!<http://user%40name@example.com/> value\n",
        ),
        (
            "!<http://example%3Aname:8080/> value\n",
            "!<http://example%3Aname:8080/> value\n",
        ),
        (
            "!<tag:example.com,2026:part%23one%23two> value\n",
            "!<tag:example.com,2026:part#one%23two> value\n",
        ),
        (
            "!<http://[2001:db8::1]:8080> value\n",
            "!<http://[2001:db8::1]:8080> value\n",
        ),
        (
            "!<http://[v1.alpha:beta]:443/path> value\n",
            "!<http://[v1.alpha:beta]:443/path> value\n",
        ),
    ] {
        let tagged: Tagged<String> = from_str(input).unwrap();
        let yaml = to_string(&tagged).unwrap();
        assert_eq!(yaml, expected);
        assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), tagged);
    }

    let ipv6: Tagged<String> = from_str("!<http://[2001:db8::1]/path%5B0%5D> value\n").unwrap();
    let yaml = to_string(&ipv6).unwrap();
    assert_eq!(yaml, "!<http://[2001:db8::1]/path%5B0%5D> value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), ipv6);
}

#[test]
fn resolved_strings_starting_with_bang_are_local_and_none_is_untagged() {
    let value = Tagged(42, Some("!!int".to_owned()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!%21int 42\n");
    assert_eq!(from_str::<Tagged<i32>>(&yaml).unwrap(), value);

    assert_eq!(
        to_string(&Tagged(42, Some("!".to_owned()))).unwrap(),
        "! 42\n"
    );
    assert_eq!(to_string(&Tagged(42, None)).unwrap(), "42\n");
    assert_eq!(from_str::<Tagged<i32>>("42\n").unwrap(), Tagged(42, None));

    let error = to_string(&Tagged(42, Some(String::new()))).unwrap_err();
    assert!(matches!(error, SerializeError::EmptyResolvedTag));
}

#[test]
fn invalid_global_tag_identity_is_rejected() {
    let error = to_string(&Tagged(1, Some("$:?".to_owned()))).unwrap_err();
    assert!(matches!(
        error,
        SerializeError::InvalidGlobalTagUri { tag } if tag == "$:?"
    ));
}

#[test]
fn tagged_flow_collections_have_correct_field_spacing_and_round_trip() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Doc {
        seq: Tagged<FlowSeq<Vec<i32>>>,
        map: Tagged<FlowMap<BTreeMap<String, i32>>>,
    }

    let doc = Doc {
        seq: Tagged(FlowSeq(vec![1, 2]), Some("!numbers".into())),
        map: Tagged(
            FlowMap(BTreeMap::from([("answer".to_owned(), 42)])),
            Some("!lookup".into()),
        ),
    };
    let yaml = to_string(&doc).unwrap();
    assert_eq!(yaml, "seq: !numbers [1, 2]\nmap: !lookup {answer: 42}\n");
    assert_eq!(from_str::<Doc>(&yaml).unwrap(), doc);
}

#[test]
fn tagged_block_collections_round_trip() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Doc {
        seq: Tagged<Vec<i32>>,
        map: Tagged<BTreeMap<String, i32>>,
    }

    let doc = Doc {
        seq: Tagged(vec![1, 2], Some("!numbers".into())),
        map: Tagged(
            BTreeMap::from([("answer".to_owned(), 42)]),
            Some("!lookup".into()),
        ),
    };
    let yaml = to_string(&doc).unwrap();
    assert!(yaml.contains("seq: !numbers\n"), "{yaml}");
    assert!(yaml.contains("map: !lookup\n"), "{yaml}");
    assert_eq!(from_str::<Doc>(&yaml).unwrap(), doc);
}

#[test]
fn tagged_empty_collections_remain_nodes_when_empty_markers_are_disabled() {
    let options = ser_options! { empty_as_braces: false };

    let seq = Tagged(Vec::<i32>::new(), Some("!empty-seq".into()));
    let yaml = to_string_with_options(&seq, options.clone()).unwrap();
    assert_eq!(yaml, "!empty-seq\n[]\n");
    assert_eq!(from_str::<Tagged<Vec<i32>>>(&yaml).unwrap(), seq);

    let map = Tagged(BTreeMap::<String, i32>::new(), Some("!empty-map".into()));
    let yaml = to_string_with_options(&map, options.clone()).unwrap();
    assert_eq!(yaml, "!empty-map\n{}\n");
    assert_eq!(
        from_str::<Tagged<BTreeMap<String, i32>>>(&yaml).unwrap(),
        map
    );

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Doc {
        seq: Tagged<Vec<i32>>,
        map: Tagged<BTreeMap<String, i32>>,
    }
    let doc = Doc { seq, map };
    let yaml = to_string_with_options(&doc, options).unwrap();
    assert_eq!(yaml, "seq: !empty-seq\n  []\nmap: !empty-map\n  {}\n");
    assert_eq!(from_str::<Doc>(&yaml).unwrap(), doc);
}

#[test]
fn tagged_and_commented_compose_in_both_orders() {
    let tagged_comment = Tagged(Commented(7, "note".into()), Some("!number".into()));
    let commented_tag = Commented(Tagged(7, Some("!number".into())), "note".into());

    assert_eq!(to_string(&tagged_comment).unwrap(), "!number 7 # note\n");
    assert_eq!(to_string(&commented_tag).unwrap(), "!number 7 # note\n");
    assert_eq!(
        from_str::<Tagged<Commented<i32>>>("!number 7 # note\n").unwrap(),
        tagged_comment
    );
    assert_eq!(
        from_str::<Commented<Tagged<i32>>>("!number 7 # note\n").unwrap(),
        commented_tag
    );
}

#[test]
fn tag_selected_enum_and_commented_compose_in_both_orders() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        Int(i32),
    }

    let tagged_comment = Tagged(Commented(Value::Int(7), "note".into()), Some("!Int".into()));
    let commented_tag = Commented(Tagged(Value::Int(7), Some("!Int".into())), "note".into());

    for yaml in [
        to_string(&tagged_comment).unwrap(),
        to_string(&commented_tag).unwrap(),
    ] {
        assert_eq!(yaml, "!Int 7 # note\n");
        assert_eq!(
            from_str::<Tagged<Commented<Value>>>(&yaml).unwrap(),
            tagged_comment
        );
        assert_eq!(
            from_str::<Commented<Tagged<Value>>>(&yaml).unwrap(),
            commented_tag
        );
    }
}

#[test]
fn equal_nested_tags_coalesce_and_different_tags_fail() {
    let same = Tagged(Tagged(7, Some("!same".into())), Some("!same".into()));
    assert_eq!(to_string(&same).unwrap(), "!same 7\n");

    let untagged = Tagged(Tagged(7, None), None);
    assert_eq!(to_string(&untagged).unwrap(), "7\n");

    let different = Tagged(Tagged(7, Some("!inner".into())), Some("!outer".into()));
    let error = to_string(&different).unwrap_err().to_string();
    assert!(
        error.contains("!outer") && error.contains("!inner"),
        "{error}"
    );

    // `None` states no requirement, so whichever side does name a tag decides.
    for deferred in [
        Tagged(Tagged(7, Some("!only".into())), None),
        Tagged(Tagged(7, None), Some("!only".into())),
    ] {
        assert_eq!(to_string(&deferred).unwrap(), "!only 7\n");
    }
}

#[test]
fn generated_enum_tags_coalesce_by_resolved_identity() {
    #[derive(Serialize)]
    enum Color {
        Red,
    }

    let options = ser_options! { tagged_enums: true };
    let same = Tagged(Color::Red, Some("tag:yaml.org,2002:Color".into()));
    assert_eq!(
        to_string_with_options(&same, options.clone()).unwrap(),
        "!<tag:yaml.org,2002:Color> Red\n"
    );

    let different = Tagged(Color::Red, Some("!other".into()));
    let error = to_string_with_options(&different, options)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("!other") && error.contains("Color"),
        "{error}"
    );

    let deferred = Tagged(Color::Red, None);
    assert_eq!(
        to_string_with_options(&deferred, ser_options! { tagged_enums: true }).unwrap(),
        "!!Color Red\n"
    );
}

/// A requested tag and a serializer-generated one meet on the same node in
/// three ways: only a differing pair is a clash.
#[test]
fn only_a_differing_generated_tag_is_a_clash() {
    #[derive(Serialize)]
    struct Doc {
        data: serde_bytes::ByteBuf,
    }
    #[derive(Serialize)]
    struct Wrapped {
        data: Tagged<serde_bytes::ByteBuf>,
    }

    let bytes = || serde_bytes::ByteBuf::from(vec![1, 2]);
    let untagged = to_string(&Doc { data: bytes() }).unwrap();
    assert_eq!(untagged, "data: !!binary AQI=\n");

    // No requested tag: the generated !!binary stands, exactly as unwrapped.
    let deferred = Wrapped {
        data: Tagged(bytes(), None),
    };
    assert_eq!(to_string(&deferred).unwrap(), untagged);

    // The same identity requested explicitly: still one tag, spelled as resolved.
    let agreeing = Wrapped {
        data: Tagged(bytes(), Some("tag:yaml.org,2002:binary".into())),
    };
    assert_eq!(
        to_string(&agreeing).unwrap(),
        "data: !<tag:yaml.org,2002:binary> AQI=\n"
    );

    // Two different identities for one node: YAML has no way to spell that.
    let clashing = Wrapped {
        data: Tagged(bytes(), Some("!other".into())),
    };
    let error = to_string(&clashing).unwrap_err().to_string();
    assert!(
        error.contains("!other") && error.contains("binary"),
        "{error}"
    );
}

/// Bytes pick their node kind from position, but the tag rules must not vary
/// with it: root and value position agree on every combination.
#[test]
fn binary_tag_rules_do_not_vary_with_position() {
    #[derive(Serialize)]
    struct Field<T> {
        data: T,
    }

    let bytes = || serde_bytes::ByteBuf::from(vec![1, 2]);

    // A root byte string stays the untagged integer sequence while no tag is staged.
    assert_eq!(to_string(&bytes()).unwrap(), "- 1\n- 2\n");
    assert_eq!(to_string(&Tagged(bytes(), None)).unwrap(), "- 1\n- 2\n");
    // Requesting the binary identity keeps the bytes on a base64 scalar instead.
    assert_eq!(
        to_string(&Tagged(bytes(), Some("tag:yaml.org,2002:binary".into()))).unwrap(),
        "!<tag:yaml.org,2002:binary> AQI=\n"
    );

    // A differing identity is a clash in both positions, not a silent change of
    // node kind at the root.
    for error in [
        to_string(&Tagged(bytes(), Some("!other".into()))).unwrap_err(),
        to_string(&Field {
            data: Tagged(bytes(), Some("!other".into())),
        })
        .unwrap_err(),
    ] {
        let error = error.to_string();
        assert!(
            error.contains("!other") && error.contains("binary"),
            "{error}"
        );
    }
}

/// `Tagged(v, None)` imposes nothing, so it cannot change how `v` is emitted.
#[test]
fn a_deferred_tag_never_alters_the_unwrapped_encoding() {
    #[derive(Serialize)]
    struct Doc<T> {
        data: T,
    }

    let bytes = || serde_bytes::ByteBuf::from(vec![1, 2]);

    // Root position: bytes at line start are a sequence of integers either way.
    assert_eq!(
        to_string(&Tagged(bytes(), None)).unwrap(),
        to_string(&bytes()).unwrap(),
    );
    // Field position: an inline base64 !!binary scalar either way.
    assert_eq!(
        to_string(&Doc {
            data: Tagged(bytes(), None)
        })
        .unwrap(),
        to_string(&Doc { data: bytes() }).unwrap(),
    );
}

#[test]
fn tag_selected_non_unit_enum_variants_serialize_as_their_payloads() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        Int(i32),
        Pair(i32, bool),
        Point { x: i32, y: i32 },
    }

    for (input, expected) in [
        ("!Int 7", "!Int 7\n"),
        ("!Pair [1, true]", "!Pair\n- 1\n- true\n"),
        ("!Point {x: 2, y: 3}", "!Point\nx: 2\n\"y\": 3\n"),
    ] {
        let value: Tagged<Value> = from_str(input).unwrap();
        let yaml = to_string(&value).unwrap();
        assert_eq!(yaml, expected);
        assert_eq!(from_str::<Tagged<Value>>(&yaml).unwrap(), value);
    }
}

#[test]
fn core_tag_selected_non_unit_enum_variants_serialize_as_their_payloads() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        Int(i32),
        Pair(i32, bool),
        Point { x: i32, y: i32 },
    }

    for (input, expected) in [
        ("!!Int 7", "!<tag:yaml.org,2002:Int> 7\n"),
        (
            "!!Pair [1, true]",
            "!<tag:yaml.org,2002:Pair>\n- 1\n- true\n",
        ),
        (
            "!!Point {x: 2, y: 3}",
            "!<tag:yaml.org,2002:Point>\nx: 2\n\"y\": 3\n",
        ),
    ] {
        let value: Tagged<Value> = from_str(input).unwrap();
        let yaml = to_string(&value).unwrap();
        assert_eq!(yaml, expected);
        assert_eq!(from_str::<Tagged<Value>>(&yaml).unwrap(), value);
    }
}

#[test]
fn bang_prefixed_tag_selected_variants_serialize_as_their_payloads() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        Int(i32),
        Pair(i32, bool),
        Point { x: i32, y: i32 },
    }

    for (input, expected) in [
        (
            "!<tag:yaml.org,2002:!Int> 7",
            "!<tag:yaml.org,2002:!Int> 7\n",
        ),
        (
            "!<tag:yaml.org,2002:!Pair> [1, true]",
            "!<tag:yaml.org,2002:!Pair>\n- 1\n- true\n",
        ),
        (
            "!<tag:yaml.org,2002:!Point> {x: 2, y: 3}",
            "!<tag:yaml.org,2002:!Point>\nx: 2\n\"y\": 3\n",
        ),
        ("!%21Int 7", "!%21Int 7\n"),
        ("!%21Pair [1, true]", "!%21Pair\n- 1\n- true\n"),
        ("!%21Point {x: 2, y: 3}", "!%21Point\nx: 2\n\"y\": 3\n"),
    ] {
        let value: Tagged<Value> = from_str(input).unwrap();
        let yaml = to_string(&value).unwrap();
        assert_eq!(yaml, expected);
        assert_eq!(from_str::<Tagged<Value>>(&yaml).unwrap(), value);
    }
}

#[test]
fn unrelated_tag_keeps_the_external_enum_wrapper() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        Int(i32),
    }

    let value = Tagged(Value::Int(7), Some("!metadata".to_owned()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!metadata\nInt: 7\n");
    assert_eq!(from_str::<Tagged<Value>>(&yaml).unwrap(), value);
}

#[test]
fn core_type_tags_cannot_replace_non_unit_enum_wrappers() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        #[serde(rename = "map")]
        Map { answer: i32 },
        #[serde(rename = "seq")]
        Seq(i32, bool),
        #[serde(rename = "int")]
        Int(i32),
        #[serde(rename = "str")]
        Str(String),
    }

    for (value, tag, variant) in [
        (Value::Map { answer: 7 }, "tag:yaml.org,2002:map", "map"),
        (Value::Seq(7, true), "tag:yaml.org,2002:seq", "seq"),
        (Value::Int(7), "tag:yaml.org,2002:int", "int"),
        (
            Value::Str("value".to_owned()),
            "tag:yaml.org,2002:str",
            "str",
        ),
    ] {
        let error = to_string(&Tagged(value, Some(tag.to_owned()))).unwrap_err();
        assert!(
            matches!(
                error,
                SerializeError::CoreTypeTagAsEnumVariant {
                    tag: error_tag,
                    variant: error_variant,
                } if error_tag == tag && error_variant == variant
            ),
            "tag: {tag}"
        );
    }
}

#[test]
fn local_map_tag_can_select_enum_variant() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Value {
        #[serde(rename = "map")]
        Map(BTreeMap<String, i32>),
    }

    let value = Tagged(
        Value::Map(BTreeMap::from([("answer".to_owned(), 7)])),
        Some("!map".to_owned()),
    );
    let yaml = to_string(&value).unwrap();

    assert_eq!(yaml, "!map\nanswer: 7\n");
    assert_eq!(from_str::<Tagged<Value>>(&yaml).unwrap(), value);
}

#[test]
fn same_tag_selects_only_one_nested_newtype_variant() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Inner {
        Value(i32),
        #[serde(other)]
        Other,
    }

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Outer {
        Value(Inner),
    }

    let value = Tagged(Outer::Value(Inner::Value(7)), Some("!Value".to_owned()));
    let yaml = to_string(&value).unwrap();

    assert_eq!(yaml, "!Value\nValue: 7\n");
    assert_eq!(from_str::<Tagged<Outer>>(&yaml).unwrap(), value);
}

#[test]
fn root_tagged_byte_buf_remains_a_binary_scalar() {
    let value: Tagged<serde_bytes::ByteBuf> = from_str("!!binary AQI=").unwrap();

    assert_eq!(
        value,
        Tagged(
            serde_bytes::ByteBuf::from(vec![1, 2]),
            Some("tag:yaml.org,2002:binary".into()),
        )
    );
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!<tag:yaml.org,2002:binary> AQI=\n");
    assert_eq!(
        from_str::<Tagged<serde_bytes::ByteBuf>>(&yaml).unwrap(),
        value
    );
}

#[test]
fn generated_binary_tag_coalesces_by_resolved_identity() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Doc {
        data: Tagged<serde_bytes::ByteBuf>,
    }

    let doc = Doc {
        data: Tagged(
            serde_bytes::ByteBuf::from(vec![1, 2]),
            Some("tag:yaml.org,2002:binary".into()),
        ),
    };
    let yaml = to_string(&doc).unwrap();
    assert_eq!(yaml, "data: !<tag:yaml.org,2002:binary> AQI=\n");
    assert_eq!(from_str::<Doc>(&yaml).unwrap(), doc);

    let conflict = Doc {
        data: Tagged(
            serde_bytes::ByteBuf::from(vec![1, 2]),
            Some("!other".into()),
        ),
    };
    let error = to_string(&conflict).unwrap_err().to_string();
    assert!(
        error.contains("!other") && error.contains("binary"),
        "{error}"
    );
}

#[test]
fn tags_on_shared_anchors_are_checked_and_do_not_leak() {
    #[derive(Deserialize, Serialize)]
    struct Doc {
        first: Tagged<RcAnchor<i32>>,
        second: Tagged<RcAnchor<i32>>,
        after: i32,
    }

    let shared = Rc::new(7);
    let doc = Doc {
        first: Tagged(RcAnchor(shared.clone()), Some("!number".into())),
        second: Tagged(RcAnchor(shared), Some("!number".into())),
        after: 9,
    };
    assert_eq!(
        to_string(&doc).unwrap(),
        "first: !number &a1 7\nsecond: *a1\nafter: 9\n"
    );

    let shared = Rc::new(7);
    let conflict = Doc {
        first: Tagged(RcAnchor(shared.clone()), Some("!first".into())),
        second: Tagged(RcAnchor(shared), Some("!second".into())),
        after: 9,
    };
    let error = to_string(&conflict).unwrap_err().to_string();
    assert!(
        error.contains("!first") && error.contains("!second"),
        "{error}"
    );

    let shared = Rc::new(7);
    let explicit_untagged = Doc {
        first: Tagged(RcAnchor(shared.clone()), None),
        second: Tagged(RcAnchor(shared), None),
        after: 9,
    };
    let yaml = to_string(&explicit_untagged).unwrap();
    assert_eq!(yaml, "first: &a1 7\nsecond: *a1\nafter: 9\n");
    let parsed: Doc = from_str(&yaml).unwrap();
    assert_eq!(parsed.first.1, None);
    assert_eq!(parsed.second.1, None);
    assert!(Rc::ptr_eq(&parsed.first.0.0, &parsed.second.0.0));

    let shared = Rc::new(7);
    let deferred = Doc {
        first: Tagged(RcAnchor(shared.clone()), Some("!number".into())),
        second: Tagged(RcAnchor(shared), None),
        after: 9,
    };
    let yaml = to_string(&deferred).unwrap();
    assert_eq!(yaml, "first: !number &a1 7\nsecond: *a1\nafter: 9\n");
    // The alias carries no tag of its own, so it reports the anchor's on the way back.
    let parsed: Doc = from_str(&yaml).unwrap();
    assert_eq!(parsed.second.1.as_deref(), Some("!number"));

    #[derive(Serialize)]
    struct UnconstrainedAlias {
        first: Tagged<RcAnchor<i32>>,
        second: RcAnchor<i32>,
    }

    let shared = Rc::new(7);
    let unconstrained = UnconstrainedAlias {
        first: Tagged(RcAnchor(shared.clone()), Some("!number".into())),
        second: RcAnchor(shared),
    };
    assert_eq!(
        to_string(&unconstrained).unwrap(),
        "first: !number &a1 7\nsecond: *a1\n"
    );

    assert_eq!(serde_json::to_string(&Tagged(7, None)).unwrap(), "7");

    #[derive(Serialize)]
    struct ComplexDoc {
        first: Tagged<RcAnchor<Vec<i32>>>,
        second: Tagged<RcAnchor<Vec<i32>>>,
    }

    let shared = Rc::new(vec![1, 2]);
    let complex = ComplexDoc {
        first: Tagged(RcAnchor(shared.clone()), Some("!numbers".into())),
        second: Tagged(RcAnchor(shared), Some("!numbers".into())),
    };
    assert_eq!(
        to_string(&complex).unwrap(),
        "first: !numbers &a1\n  - 1\n  - 2\nsecond: *a1\n"
    );
}

#[test]
fn tag_text_cannot_inject_yaml_syntax() {
    let value = Tagged("value", Some("tag:evil>\nforged: true #".into()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml.lines().count(), 1, "{yaml:?}");
    assert!(!yaml.contains("\nforged"), "{yaml:?}");
    // `#` is a legal URI character inside `!<...>` and cannot begin a YAML
    // comment there; the closing bracket and newline are percent encoded.
    assert!(yaml.contains("%3E%0Aforged:%20true%20#>"), "{yaml:?}");
}
