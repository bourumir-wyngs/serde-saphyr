#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use serde_saphyr::{
    CommentPosition, Commented, FlowMap, FlowSeq, RcAnchor, Spanned, ser_options, to_string,
    to_string_with_options,
};

#[test]
fn commented_scalar_block_style() {
    let y = to_string(&Commented(42, "answer".to_string())).unwrap();
    assert_eq!(y, "42 # answer\n");
}

#[test]
fn commented_scalar_as_map_value_inline() {
    #[derive(Serialize)]
    struct Wrap {
        k: Commented<i32>,
    }
    let v = Wrap {
        k: Commented(5, "hi".into()),
    };
    let y = to_string(&v).unwrap();
    assert_eq!(y, "k: 5 # hi\n");
}

#[test]
fn commented_scalar_suppressed_in_flow_seq() {
    let seq = FlowSeq(vec![Commented(1, "a".to_string()), Commented(2, "".into())]);
    let y = to_string(&seq).unwrap();
    // Comments are suppressed in flow contexts
    assert_eq!(y, "[1, 2]\n");
}

#[test]
fn commented_scalar_suppressed_in_flow_map_value() {
    let mut m: HashMap<&str, Commented<i32>> = HashMap::new();
    m.insert("a", Commented(1, "x".into()));
    m.insert("b", Commented(2, "y".into()));
    let y = to_string(&FlowMap(m)).unwrap();
    // No comments inside flow mapping
    // HashMap iteration order is undefined; parse back to verify structurally and check absence of '#'
    assert!(y.starts_with("{") && y.ends_with("}\n"));
    assert!(!y.contains('#'));
}

#[test]
fn commented_complex_values() {
    let y = to_string(&Commented(vec![1, 2], "ignored".into())).unwrap();
    assert_eq!(y, "- 1\n- 2\n");
}

#[test]
fn commented_scalar_block_style_above() {
    let y = to_string_with_options(
        &Commented(42, "answer".to_string()),
        ser_options! { comment_position: CommentPosition::Above },
    )
    .unwrap();
    assert_eq!(y, "# answer\n42\n");
}

#[test]
fn commented_scalar_as_map_value_above() {
    #[derive(Serialize)]
    struct Wrap {
        k: Commented<i32>,
    }

    let y = to_string_with_options(
        &Wrap {
            k: Commented(5, "hi".into()),
        },
        ser_options! { comment_position: CommentPosition::Above },
    )
    .unwrap();
    assert_eq!(y, "k:\n  # hi\n  5\n");
}

#[test]
fn commented_scalar_as_seq_item_above() {
    let y = to_string_with_options(
        &vec![Commented(5, "hi".into())],
        ser_options! { comment_position: CommentPosition::Above },
    )
    .unwrap();
    assert_eq!(y, "- \n  # hi\n  5\n");
}

#[test]
fn commented_complex_value_above() {
    let y = to_string_with_options(
        &Commented(vec![1, 2], "items".into()),
        ser_options! { comment_position: CommentPosition::Above },
    )
    .unwrap();
    assert_eq!(y, "# items\n- 1\n- 2\n");
}

#[test]
fn commented_newlines_are_sanitized() {
    let y = to_string(&Commented(7, "line1\nline2".into())).unwrap();
    assert_eq!(y, "7 # line1 line2\n");
}

#[test]
fn commented_carriage_returns_are_sanitized() {
    let y = to_string(&Commented(7, "line1\rline2".into())).unwrap();
    assert_eq!(y, "7 # line1 line2\n");
}

#[test]
fn commented_above_sanitizes_newlines() {
    let y = to_string_with_options(
        &Commented(7, "line1\nline2".into()),
        ser_options! { comment_position: CommentPosition::Above },
    )
    .unwrap();
    assert_eq!(y, "# line1 line2\n7\n");
}

#[test]
fn commented_scalar_suppressed_in_flow_seq_above() {
    let seq = FlowSeq(vec![Commented(1, "a".to_string()), Commented(2, "".into())]);
    let y = to_string_with_options(
        &seq,
        ser_options! { comment_position: CommentPosition::Above },
    )
    .unwrap();
    assert_eq!(y, "[1, 2]\n");
}

#[test]
fn commented_deserialize_captures_inline_comment_and_keeps_value() {
    let input = "5 # whatever\n";
    let v: Commented<i32> = serde_saphyr::from_str(input).unwrap();
    assert_eq!(v.0, 5);
    assert_eq!(v.1, "whatever");

    let v3: Commented<i32> = serde_saphyr::from_str("# root\n5\n").unwrap();
    assert_eq!(v3, Commented(5, "root".to_string()));

    // Also round-trip without comment in input
    let v2: Commented<i32> = serde_saphyr::from_str("5\n").unwrap();
    assert_eq!(v2.0, 5);
    assert!(v2.1.is_empty());
}

#[test]
fn commented_deserialize_captures_comments_around_mapping_fields() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Wrap {
        first: Commented<i32>,
        second: Commented<i32>,
        third: Commented<i32>,
    }

    let input = "\
# first field
first: 1
second:
  # second value
  2
third: # third separator
  3
";
    let value: Wrap = serde_saphyr::from_str(input).unwrap();

    assert_eq!(value.first, Commented(1, "first field".to_string()));
    assert_eq!(value.second, Commented(2, "second value".to_string()));
    assert_eq!(value.third, Commented(3, "third separator".to_string()));
}

#[test]
fn commented_deserialize_inside_spanned_preserves_field_comment() {
    #[derive(Debug, Deserialize)]
    struct Wrap {
        x: Spanned<Commented<i32>>,
    }

    for yaml in ["# note\nx: 1\n", "x: # note\n  1\n", "x:\n  # note\n  1\n"] {
        let value: Wrap = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(value.x.value, Commented(1, "note".to_string()));
    }
}

#[test]
fn commented_deserialize_captures_sequence_item_comments_without_leaking() {
    let input = "- 1 # one\n- 2 # two\n- 3\n";
    let value: Vec<Commented<i32>> = serde_saphyr::from_str(input).unwrap();

    assert_eq!(value[0], Commented(1, "one".to_string()));
    assert_eq!(value[1], Commented(2, "two".to_string()));
    assert_eq!(value[2], Commented(3, String::new()));
}

#[test]
fn commented_deserialize_captures_sequence_dash_comment_before_value() {
    let input = "- # one\n  1\n- # two\n  2\n";
    let value: Vec<Commented<i32>> = serde_saphyr::from_str(input).unwrap();

    assert_eq!(value[0], Commented(1, "one".to_string()));
    assert_eq!(value[1], Commented(2, "two".to_string()));
}

#[test]
fn commented_deserialize_keeps_trailing_and_dash_comments_separate() {
    let input = "- 1 # one\n- # two\n  2\n";
    let value: Vec<Commented<i32>> = serde_saphyr::from_str(input).unwrap();

    assert_eq!(value[0], Commented(1, "one".to_string()));
    assert_eq!(value[1], Commented(2, "two".to_string()));
}

#[test]
fn commented_deserialize_does_not_leak_parent_comments_into_nested_values() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        inner: Inner,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Inner {
        field: Commented<i32>,
    }

    let value: Outer = serde_saphyr::from_str("# inner object\ninner:\n  field: 1\n").unwrap();
    assert_eq!(value.inner.field, Commented(1, String::new()));

    let value: Outer = serde_saphyr::from_str("inner:\n  # actual field\n  field: 1\n").unwrap();
    assert_eq!(value.inner.field, Commented(1, "actual field".to_string()));
}

#[test]
fn commented_deserialize_does_not_leak_parent_separator_comment_into_nested_map_field() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        inner: Inner,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Inner {
        field: Commented<i32>,
    }

    let value: Outer = serde_saphyr::from_str("inner: # inner object\n  field: 1\n").unwrap();

    assert_eq!(value.inner.field, Commented(1, String::new()));
}

#[test]
fn commented_deserialize_container_comment_is_not_inherited_by_first_child_key() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        root: Commented<Inner>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Inner {
        child: Commented<i32>,
    }

    let value: Outer = serde_saphyr::from_str("root: # root container\n  child: 1\n").unwrap();

    assert_eq!(value.root.1, "root container");
    assert_eq!(value.root.0.child, Commented(1, String::new()));
}

#[test]
fn commented_deserialize_child_key_comment_is_captured_by_child() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        root: Commented<Inner>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Inner {
        child: Commented<i32>,
    }

    let value: Outer = serde_saphyr::from_str("root:\n  # child\n  child: 1\n").unwrap();

    assert_eq!(value.root.1, "");
    assert_eq!(value.root.0.child, Commented(1, "child".to_string()));
}

#[test]
fn commented_deserialize_sequence_container_comment_is_not_inherited_by_first_element() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        root: Commented<Vec<Commented<i32>>>,
    }

    let value: Outer = serde_saphyr::from_str("root: # root container\n  - 1\n").unwrap();

    assert_eq!(value.root.1, "root container");
    assert_eq!(value.root.0[0], Commented(1, String::new()));
}

#[test]
fn commented_deserialize_sequence_element_comment_is_captured_by_element() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        root: Commented<Vec<Commented<i32>>>,
    }

    let value: Outer = serde_saphyr::from_str("root:\n  # item\n  - 1\n").unwrap();

    assert_eq!(value.root.1, "");
    assert_eq!(value.root.0[0], Commented(1, "item".to_string()));
}

#[test]
fn test_commented_rc() -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Notable {
        value: usize,
        notable_value: Commented<RcAnchor<usize>>,
    }

    let notable = Notable {
        value: 127,
        notable_value: Commented(RcAnchor::wrapping(541), "comment".to_string()),
    };

    let yaml = serde_saphyr::to_string(&notable)?;
    assert!(yaml.contains("127"));
    assert!(yaml.contains("541"));
    assert!(yaml.contains("comment"));
    Ok(())
}
