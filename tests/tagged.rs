#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_saphyr::{
    Commented, FlowMap, FlowSeq, RcAnchor, Tagged, from_str, ser_options, to_string,
    to_string_with_options,
};

#[test]
fn resolved_core_verbatim_and_directive_tags_normalize_on_output() {
    let core: Tagged<i32> = from_str("!!int 42").unwrap();
    assert_eq!(core, Tagged(42, "tag:yaml.org,2002:int".into()));
    let yaml = to_string(&core).unwrap();
    assert_eq!(yaml, "!<tag:yaml.org,2002:int> 42\n");
    assert_eq!(from_str::<Tagged<i32>>(&yaml).unwrap(), core);

    let verbatim: Tagged<String> = from_str("!<tag:example.com,2026:widget> value").unwrap();
    assert_eq!(verbatim.1, "tag:example.com,2026:widget");
    let yaml = to_string(&verbatim).unwrap();
    assert_eq!(yaml, "!<tag:example.com,2026:widget> value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), verbatim);

    let directive: Tagged<String> =
        from_str("%TAG !e! tag:example.com,2026:\n--- !e!widget value\n").unwrap();
    assert_eq!(directive.1, "tag:example.com,2026:widget");
    let yaml = to_string(&directive).unwrap();
    assert_eq!(yaml, "!<tag:example.com,2026:widget> value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), directive);
}

#[test]
fn local_and_global_resolved_tags_are_safely_percent_encoded() {
    let local = Tagged("value".to_owned(), "!space here/%".to_owned());
    let yaml = to_string(&local).unwrap();
    assert_eq!(yaml, "!space%20here/%25 value\n");
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), local);

    let global = Tagged(
        "value".to_owned(),
        "tag:example.com,2026:a>b % snowman ☃".to_owned(),
    );
    let yaml = to_string(&global).unwrap();
    assert_eq!(
        yaml,
        "!<tag:example.com,2026:a%3Eb%20%25%20snowman%20%E2%98%83> value\n"
    );
    assert_eq!(from_str::<Tagged<String>>(&yaml).unwrap(), global);
}

#[test]
fn resolved_strings_starting_with_bang_are_local() {
    let value = Tagged(42, "!!int".to_owned());
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!%21int 42\n");
    assert_eq!(from_str::<Tagged<i32>>(&yaml).unwrap(), value);

    assert_eq!(to_string(&Tagged(42, "!".to_owned())).unwrap(), "! 42\n");
    assert_eq!(to_string(&Tagged(42, String::new())).unwrap(), "42\n");
}

#[test]
fn tagged_flow_collections_have_correct_field_spacing_and_round_trip() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Doc {
        seq: Tagged<FlowSeq<Vec<i32>>>,
        map: Tagged<FlowMap<BTreeMap<String, i32>>>,
    }

    let doc = Doc {
        seq: Tagged(FlowSeq(vec![1, 2]), "!numbers".into()),
        map: Tagged(
            FlowMap(BTreeMap::from([("answer".to_owned(), 42)])),
            "!lookup".into(),
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
        seq: Tagged(vec![1, 2], "!numbers".into()),
        map: Tagged(
            BTreeMap::from([("answer".to_owned(), 42)]),
            "!lookup".into(),
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

    let seq = Tagged(Vec::<i32>::new(), "!empty-seq".into());
    let yaml = to_string_with_options(&seq, options.clone()).unwrap();
    assert_eq!(yaml, "!empty-seq\n[]\n");
    assert_eq!(from_str::<Tagged<Vec<i32>>>(&yaml).unwrap(), seq);

    let map = Tagged(BTreeMap::<String, i32>::new(), "!empty-map".into());
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
    let tagged_comment = Tagged(Commented(7, "note".into()), "!number".into());
    let commented_tag = Commented(Tagged(7, "!number".into()), "note".into());

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
fn equal_nested_tags_coalesce_and_different_tags_fail() {
    let same = Tagged(Tagged(7, "!same".into()), "!same".into());
    assert_eq!(to_string(&same).unwrap(), "!same 7\n");

    let different = Tagged(Tagged(7, "!inner".into()), "!outer".into());
    let error = to_string(&different).unwrap_err().to_string();
    assert!(
        error.contains("!outer") && error.contains("!inner"),
        "{error}"
    );
}

#[test]
fn generated_enum_tags_coalesce_by_resolved_identity() {
    #[derive(Serialize)]
    enum Color {
        Red,
    }

    let options = ser_options! { tagged_enums: true };
    let same = Tagged(Color::Red, "tag:yaml.org,2002:Color".into());
    assert_eq!(
        to_string_with_options(&same, options.clone()).unwrap(),
        "!<tag:yaml.org,2002:Color> Red\n"
    );

    let different = Tagged(Color::Red, "!other".into());
    let error = to_string_with_options(&different, options)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("!other") && error.contains("Color"),
        "{error}"
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
            "tag:yaml.org,2002:binary".into(),
        ),
    };
    let yaml = to_string(&doc).unwrap();
    assert_eq!(yaml, "data: !<tag:yaml.org,2002:binary> AQI=\n");
    assert_eq!(from_str::<Doc>(&yaml).unwrap(), doc);

    let conflict = Doc {
        data: Tagged(serde_bytes::ByteBuf::from(vec![1, 2]), "!other".into()),
    };
    let error = to_string(&conflict).unwrap_err().to_string();
    assert!(
        error.contains("!other") && error.contains("binary"),
        "{error}"
    );
}

#[test]
fn tags_on_shared_anchors_are_checked_and_do_not_leak() {
    #[derive(Serialize)]
    struct Doc {
        first: Tagged<RcAnchor<i32>>,
        second: Tagged<RcAnchor<i32>>,
        after: i32,
    }

    let shared = Rc::new(7);
    let doc = Doc {
        first: Tagged(RcAnchor(shared.clone()), "!number".into()),
        second: Tagged(RcAnchor(shared), "!number".into()),
        after: 9,
    };
    assert_eq!(
        to_string(&doc).unwrap(),
        "first: !number &a1 7\nsecond: *a1\nafter: 9\n"
    );

    let shared = Rc::new(7);
    let conflict = Doc {
        first: Tagged(RcAnchor(shared.clone()), "!first".into()),
        second: Tagged(RcAnchor(shared), "!second".into()),
        after: 9,
    };
    let error = to_string(&conflict).unwrap_err().to_string();
    assert!(
        error.contains("!first") && error.contains("!second"),
        "{error}"
    );
}

#[test]
fn tag_text_cannot_inject_yaml_syntax() {
    let value = Tagged("value", "tag:evil>\nforged: true #".into());
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml.lines().count(), 1, "{yaml:?}");
    assert!(!yaml.contains("\nforged"), "{yaml:?}");
    // `#` is a legal URI character inside `!<...>` and cannot begin a YAML
    // comment there; the closing bracket and newline are percent encoded.
    assert!(yaml.contains("%3E%0Aforged:%20true%20#>"), "{yaml:?}");
}
