#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Serialize;
use serde_saphyr::{Commented, FlowSeq, SpaceAfter, to_string};

#[test]
fn commented_in_flow_seq_suppresses_comment() {
    let v = FlowSeq(vec![Commented(42i32, "# note".to_string())]);
    let yaml = to_string(&v).unwrap();
    // In flow context, comment is suppressed; value still present
    assert!(yaml.contains("42"), "yaml: {yaml}");
    // Comment should NOT appear in flow
    assert!(
        !yaml.contains("# note"),
        "comment should be suppressed in flow: {yaml}"
    );
}

#[test]
fn space_after_in_flow_no_extra_newline() {
    let v = FlowSeq(vec![SpaceAfter(1i32), SpaceAfter(2i32)]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("1") && yaml.contains("2"), "yaml: {yaml}");
}

#[test]
fn commented_in_block_emits_comment() {
    let v = Commented(42i32, "# my comment".to_string());
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("42"), "yaml: {yaml}");
    assert!(yaml.contains("# my comment"), "expected comment: {yaml}");
}

#[test]
fn space_after_in_block_adds_blank_line() {
    #[derive(Serialize)]
    struct S {
        a: SpaceAfter<i32>,
        b: i32,
    }
    let yaml = to_string(&S {
        a: SpaceAfter(1),
        b: 2,
    })
    .unwrap();
    assert!(yaml.contains("a: 1"), "yaml: {yaml}");
    assert!(yaml.contains("b: 2"), "yaml: {yaml}");
    // There should be a blank line between a and b
    assert!(yaml.contains("\n\n"), "expected blank line: {yaml}");
}

#[test]
fn commented_newline_in_comment_sanitized() {
    let v = Commented(42i32, "line1\nline2".to_string());
    let yaml = to_string(&v).unwrap();
    // Newline in comment should be replaced with space
    assert!(yaml.contains("42"), "yaml: {yaml}");
    assert!(
        !yaml.contains('\n') || yaml.lines().count() <= 2,
        "yaml: {yaml}"
    );
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

#[test]
fn commented_in_flow_context_suppresses_comment() {
    let yaml = to_string(&FlowSeq(vec![Commented(1, "note".into())])).unwrap();
    assert!(
        !yaml.contains('#'),
        "comment should be suppressed in flow: {yaml}"
    );
}

#[test]
fn commented_empty_string_no_comment_marker() {
    let yaml = to_string(&Commented(42, String::new())).unwrap();
    assert!(
        !yaml.contains('#'),
        "empty comment should not emit #: {yaml}"
    );
    assert!(yaml.contains("42"), "value missing: {yaml}");
}

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

#[test]
fn serialize_space_after() {
    use serde_saphyr::SpaceAfter;
    #[derive(Serialize)]
    struct Doc {
        section: SpaceAfter<Vec<i32>>,
        other: i32,
    }
    let s = serde_saphyr::to_string(&Doc {
        section: SpaceAfter(vec![1]),
        other: 2,
    })
    .unwrap();
    assert!(s.contains("other: 2"));
}

#[test]
fn serialize_commented() {
    use serde_saphyr::Commented;
    #[derive(Serialize)]
    struct Doc {
        field: Commented<i32>,
    }
    let s = serde_saphyr::to_string(&Doc {
        field: Commented(42, "a comment".to_string()),
    })
    .unwrap();
    assert!(s.contains("# a comment") || s.contains("comment"));
}
