use std::rc::Rc;
use std::sync::Arc;

use indoc::indoc;
use serde::Deserialize;
use serde_saphyr::{from_str, ArcAnchor, ArcWeakAnchor, RcAnchor, RcWeakAnchor};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Node {
    name: String,
}

#[derive(Deserialize)]
struct RcDoc {
    strong: RcAnchor<Node>,
    weak: RcWeakAnchor<Node>,
}

#[derive(Deserialize)]
struct ArcDoc {
    strong: ArcAnchor<Node>,
    weak: ArcWeakAnchor<Node>,
}

#[test]
fn rc_weak_anchor_deserialize_success() {
    let y = indoc! {r#"
        strong: &a1
          name: primary
        weak: *a1
    "#};
    let doc: RcDoc = from_str(y).expect("rc weak should deserialize when strong defined earlier");

    // Upgrading weak should succeed and point to the exact same allocation as strong.
    let upgraded = doc.weak.upgrade().expect("weak should upgrade");
    assert!(Rc::ptr_eq(&upgraded, &doc.strong.0));
}

#[test]
fn arc_weak_anchor_deserialize_success() {
    let y = indoc! {r#"
        strong: &a1
          name: primary
        weak: *a1
    "#};
    let doc: ArcDoc = from_str(y).expect("arc weak should deserialize when strong defined earlier");

    let upgraded = doc.weak.upgrade().expect("weak should upgrade");
    assert!(Arc::ptr_eq(&upgraded, &doc.strong.0));
}

#[test]
fn rc_weak_anchor_before_strong_should_error() {
    // Weak alias refers to a1 before it is defined → parser/expander must error.
    let y = indoc! {r#"
        weak: *a1
        strong: &a1
          name: later
    "#};
    let res: Result<RcDoc, _> = from_str(y);
    assert!(res.is_err(), "expected error when weak alias appears before strong anchor");
}

#[test]
fn arc_weak_anchor_non_alias_should_error() {
    // A weak anchor must be an alias to an existing anchor; providing a fresh mapping is invalid.
    let y = indoc! {r#"
        strong: &a1
          name: primary
        weak:
          name: not-an-alias
    "#};
    let res: Result<ArcDoc, _> = from_str(y);
    assert!(res.is_err(), "expected error when weak value is not an alias");
}
