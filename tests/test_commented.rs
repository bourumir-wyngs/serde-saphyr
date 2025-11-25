use std::collections::HashMap;
use serde::Serialize;

use serde_saphyr::{to_string, Commented, FlowMap, FlowSeq, RcAnchor};

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
    let v = Wrap { k: Commented(5, "hi".into()) };
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
fn commented_newlines_are_sanitized() {
    let y = to_string(&Commented(7, "line1\nline2".into())).unwrap();
    assert_eq!(y, "7 # line1 line2\n");
}

#[test]
fn commented_deserialize_ignores_comment_and_keeps_value() {
    // Even if the source contains a YAML comment, deserialization into Commented<T>
    // should yield the inner T and an empty comment string.
    let input = "5 # whatever\n";
    let v: Commented<i32> = serde_saphyr::from_str(input).unwrap();
    assert_eq!(v.0, 5);
    assert!(v.1.is_empty());

    // Also round-trip without comment in input
    let v2: Commented<i32> = serde_saphyr::from_str("5\n").unwrap();
    assert_eq!(v2.0, 5);
    assert!(v2.1.is_empty());
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
        notable_value: Commented(RcAnchor::wrapping(541), "comment".to_string())
    };

    let yaml = serde_saphyr::to_string(&notable)?;

    println!("{yaml}");
    assert!(yaml.contains("127"));
    assert!(yaml.contains("541"));
    assert!(yaml.contains("comment"));
    Ok(())
}