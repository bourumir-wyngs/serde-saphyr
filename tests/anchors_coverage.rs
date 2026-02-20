//! Tests targeting coverage gaps in `src/anchors.rs`.
//! Covers: From conversions, Deref/AsRef/Borrow/Into, PartialEq/Eq,
//! Debug, Default, wrapping constructors, weak helpers, and Deserialize
//! error paths for all anchor types.

use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use indoc::indoc;
use serde::{Deserialize, Serialize};
use serde_saphyr::{
    ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor, RcAnchor, RcRecursion, RcRecursive,
    RcWeakAnchor,
};

// ── shared helper types ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
struct Val {
    x: i32,
}

// ── From conversions ─────────────────────────────────────────────────────────

#[test]
fn from_rc_owned_into_rc_weak_anchor() {
    let rc = Rc::new(Val { x: 1 });
    let weak: RcWeakAnchor<Val> = RcWeakAnchor::from(rc.clone());
    assert!(weak.upgrade().is_some());
    drop(rc);
    assert!(weak.upgrade().is_none());
}

#[test]
fn from_arc_ref_into_arc_weak_anchor() {
    let arc = Arc::new(Val { x: 2 });
    let weak: ArcWeakAnchor<Val> = ArcWeakAnchor::from(&arc);
    assert!(weak.upgrade().is_some());
}

#[test]
fn from_arc_owned_into_arc_weak_anchor() {
    let arc = Arc::new(Val { x: 3 });
    let weak: ArcWeakAnchor<Val> = ArcWeakAnchor::from(arc.clone());
    assert!(weak.upgrade().is_some());
    drop(arc);
    assert!(weak.upgrade().is_none());
}

#[test]
fn from_arc_anchor_ref_into_arc_weak_anchor() {
    let anchor = ArcAnchor::from(Arc::new(Val { x: 4 }));
    let weak: ArcWeakAnchor<Val> = ArcWeakAnchor::from(&anchor);
    assert!(weak.upgrade().is_some());
}

#[test]
fn from_rc_anchor_into_rc() {
    let anchor = RcAnchor::from(Rc::new(Val { x: 5 }));
    let rc: Rc<Val> = Rc::from(anchor);
    assert_eq!(rc.x, 5);
}

#[test]
fn from_arc_anchor_into_arc() {
    let anchor = ArcAnchor::from(Arc::new(Val { x: 6 }));
    let arc: Arc<Val> = Arc::from(anchor);
    assert_eq!(arc.x, 6);
}

// ── Deref / AsRef / Borrow ───────────────────────────────────────────────────

#[test]
fn rc_recursive_deref() {
    let r = RcRecursive::wrapping(Val { x: 7 });
    let inner: &Rc<std::cell::RefCell<Option<Val>>> = r.deref();
    assert!(inner.as_ref().borrow().as_ref().unwrap().x == 7);
}

#[test]
fn arc_recursive_deref() {
    let r = ArcRecursive::wrapping(Val { x: 8 });
    let inner: &Arc<std::sync::Mutex<Option<Val>>> = r.deref();
    assert!(inner.lock().unwrap().as_ref().unwrap().x == 8);
}

#[test]
fn rc_anchor_as_ref() {
    let anchor = RcAnchor::from(Rc::new(Val { x: 9 }));
    let rc_ref: &Rc<Val> = anchor.as_ref();
    assert_eq!(rc_ref.x, 9);
}

#[test]
fn arc_anchor_as_ref() {
    let anchor = ArcAnchor::from(Arc::new(Val { x: 10 }));
    let arc_ref: &Arc<Val> = anchor.as_ref();
    assert_eq!(arc_ref.x, 10);
}

#[test]
fn rc_anchor_borrow() {
    use std::borrow::Borrow;
    let anchor = RcAnchor::from(Rc::new(Val { x: 11 }));
    let rc_ref: &Rc<Val> = anchor.borrow();
    assert_eq!(rc_ref.x, 11);
}

#[test]
fn arc_anchor_borrow() {
    use std::borrow::Borrow;
    let anchor = ArcAnchor::from(Arc::new(Val { x: 12 }));
    let arc_ref: &Arc<Val> = anchor.borrow();
    assert_eq!(arc_ref.x, 12);
}

// ── PartialEq / Eq ───────────────────────────────────────────────────────────

#[test]
fn rc_anchor_eq_same_ptr() {
    let rc = Rc::new(Val { x: 1 });
    let a = RcAnchor(rc.clone());
    let b = RcAnchor(rc.clone());
    assert_eq!(a, b);
}

#[test]
fn rc_anchor_ne_different_ptr() {
    let a = RcAnchor(Rc::new(Val { x: 1 }));
    let b = RcAnchor(Rc::new(Val { x: 1 }));
    assert_ne!(a, b);
}

#[test]
fn arc_anchor_eq_same_ptr() {
    let arc = Arc::new(Val { x: 2 });
    let a = ArcAnchor(arc.clone());
    let b = ArcAnchor(arc.clone());
    assert_eq!(a, b);
}

#[test]
fn arc_anchor_ne_different_ptr() {
    let a = ArcAnchor(Arc::new(Val { x: 2 }));
    let b = ArcAnchor(Arc::new(Val { x: 2 }));
    assert_ne!(a, b);
}

#[test]
fn rc_weak_anchor_eq_same_ptr() {
    let rc = Rc::new(Val { x: 3 });
    let a = RcWeakAnchor::from(&rc);
    let b = RcWeakAnchor::from(&rc);
    assert_eq!(a, b);
}

#[test]
fn rc_weak_anchor_ne_different_ptr() {
    let rc1 = Rc::new(Val { x: 3 });
    let rc2 = Rc::new(Val { x: 3 });
    let a = RcWeakAnchor::from(&rc1);
    let b = RcWeakAnchor::from(&rc2);
    assert_ne!(a, b);
}

#[test]
fn rc_weak_anchor_eq_both_dangling() {
    let a: RcWeakAnchor<Val> = RcWeakAnchor(Rc::downgrade(&{
        let tmp = Rc::new(Val { x: 0 });
        tmp
    }));
    let b: RcWeakAnchor<Val> = RcWeakAnchor(Rc::downgrade(&{
        let tmp = Rc::new(Val { x: 0 });
        tmp
    }));
    // Both dangling → equal
    assert_eq!(a, b);
}

#[test]
fn rc_weak_anchor_ne_one_dangling() {
    let rc = Rc::new(Val { x: 3 });
    let alive = RcWeakAnchor::from(&rc);
    let dangling: RcWeakAnchor<Val> = RcWeakAnchor(Rc::downgrade(&{
        let tmp = Rc::new(Val { x: 0 });
        tmp
    }));
    assert_ne!(alive, dangling);
}

#[test]
fn arc_weak_anchor_eq_same_ptr() {
    let arc = Arc::new(Val { x: 4 });
    let a = ArcWeakAnchor::from(&arc);
    let b = ArcWeakAnchor::from(&arc);
    assert_eq!(a, b);
}

#[test]
fn arc_weak_anchor_ne_different_ptr() {
    let arc1 = Arc::new(Val { x: 4 });
    let arc2 = Arc::new(Val { x: 4 });
    let a = ArcWeakAnchor::from(&arc1);
    let b = ArcWeakAnchor::from(&arc2);
    assert_ne!(a, b);
}

#[test]
fn arc_weak_anchor_eq_both_dangling() {
    let a: ArcWeakAnchor<Val> = ArcWeakAnchor(Arc::downgrade(&{
        let tmp = Arc::new(Val { x: 0 });
        tmp
    }));
    let b: ArcWeakAnchor<Val> = ArcWeakAnchor(Arc::downgrade(&{
        let tmp = Arc::new(Val { x: 0 });
        tmp
    }));
    assert_eq!(a, b);
}

#[test]
fn arc_weak_anchor_ne_one_dangling() {
    let arc = Arc::new(Val { x: 4 });
    let alive = ArcWeakAnchor::from(&arc);
    let dangling: ArcWeakAnchor<Val> = ArcWeakAnchor(Arc::downgrade(&{
        let tmp = Arc::new(Val { x: 0 });
        tmp
    }));
    assert_ne!(alive, dangling);
}

#[test]
fn rc_recursive_eq_same_ptr() {
    let r = RcRecursive::wrapping(Val { x: 5 });
    let r2 = r.clone();
    assert_eq!(r, r2);
}

#[test]
fn rc_recursive_ne_different_ptr() {
    let a = RcRecursive::wrapping(Val { x: 5 });
    let b = RcRecursive::wrapping(Val { x: 5 });
    assert_ne!(a, b);
}

#[test]
fn arc_recursive_eq_same_ptr() {
    let r = ArcRecursive::wrapping(Val { x: 6 });
    let r2 = r.clone();
    assert_eq!(r, r2);
}

#[test]
fn arc_recursive_ne_different_ptr() {
    let a = ArcRecursive::wrapping(Val { x: 6 });
    let b = ArcRecursive::wrapping(Val { x: 6 });
    assert_ne!(a, b);
}

#[test]
fn rc_recursion_eq_same_ptr() {
    let r = RcRecursive::wrapping(Val { x: 7 });
    let a = RcRecursion::from(&r);
    let b = RcRecursion::from(&r);
    assert_eq!(a, b);
}

#[test]
fn rc_recursion_ne_different_ptr() {
    let r1 = RcRecursive::wrapping(Val { x: 7 });
    let r2 = RcRecursive::wrapping(Val { x: 7 });
    let a = RcRecursion::from(&r1);
    let b = RcRecursion::from(&r2);
    assert_ne!(a, b);
}

#[test]
fn rc_recursion_eq_both_dangling() {
    let a = {
        let r = RcRecursive::wrapping(Val { x: 0 });
        RcRecursion::from(&r)
    };
    let b = {
        let r = RcRecursive::wrapping(Val { x: 0 });
        RcRecursion::from(&r)
    };
    assert_eq!(a, b);
}

#[test]
fn rc_recursion_ne_one_dangling() {
    let r = RcRecursive::wrapping(Val { x: 7 });
    let alive = RcRecursion::from(&r);
    let dangling = {
        let tmp = RcRecursive::wrapping(Val { x: 0 });
        RcRecursion::from(&tmp)
    };
    assert_ne!(alive, dangling);
}

#[test]
fn arc_recursion_eq_same_ptr() {
    let r = ArcRecursive::wrapping(Val { x: 8 });
    let a = ArcRecursion::from(&r);
    let b = ArcRecursion::from(&r);
    assert_eq!(a, b);
}

#[test]
fn arc_recursion_ne_different_ptr() {
    let r1 = ArcRecursive::wrapping(Val { x: 8 });
    let r2 = ArcRecursive::wrapping(Val { x: 8 });
    let a = ArcRecursion::from(&r1);
    let b = ArcRecursion::from(&r2);
    assert_ne!(a, b);
}

#[test]
fn arc_recursion_eq_both_dangling() {
    let a = {
        let r = ArcRecursive::wrapping(Val { x: 0 });
        ArcRecursion::from(&r)
    };
    let b = {
        let r = ArcRecursive::wrapping(Val { x: 0 });
        ArcRecursion::from(&r)
    };
    assert_eq!(a, b);
}

#[test]
fn arc_recursion_ne_one_dangling() {
    let r = ArcRecursive::wrapping(Val { x: 8 });
    let alive = ArcRecursion::from(&r);
    let dangling = {
        let tmp = ArcRecursive::wrapping(Val { x: 0 });
        ArcRecursion::from(&tmp)
    };
    assert_ne!(alive, dangling);
}

// ── Debug ────────────────────────────────────────────────────────────────────

#[test]
fn debug_rc_anchor() {
    let a = RcAnchor(Rc::new(Val { x: 1 }));
    let s = format!("{:?}", a);
    assert!(s.starts_with("RcAnchor(0x"), "got: {s}");
}

#[test]
fn debug_arc_anchor() {
    let a = ArcAnchor(Arc::new(Val { x: 1 }));
    let s = format!("{:?}", a);
    assert!(s.starts_with("ArcAnchor(0x"), "got: {s}");
}

#[test]
fn debug_rc_weak_anchor_alive() {
    let rc = Rc::new(Val { x: 1 });
    let w = RcWeakAnchor::from(&rc);
    let s = format!("{:?}", w);
    assert!(s.contains("upgrade="), "got: {s}");
}

#[test]
fn debug_rc_weak_anchor_dangling() {
    let w: RcWeakAnchor<Val> = RcWeakAnchor(Rc::downgrade(&{
        let tmp = Rc::new(Val { x: 0 });
        tmp
    }));
    let s = format!("{:?}", w);
    assert!(s.contains("dangling"), "got: {s}");
}

#[test]
fn debug_arc_weak_anchor_alive() {
    let arc = Arc::new(Val { x: 1 });
    let w = ArcWeakAnchor::from(&arc);
    let s = format!("{:?}", w);
    assert!(s.contains("upgrade="), "got: {s}");
}

#[test]
fn debug_arc_weak_anchor_dangling() {
    let w: ArcWeakAnchor<Val> = ArcWeakAnchor(Arc::downgrade(&{
        let tmp = Arc::new(Val { x: 0 });
        tmp
    }));
    let s = format!("{:?}", w);
    assert!(s.contains("dangling"), "got: {s}");
}

#[test]
fn debug_rc_recursive() {
    let r = RcRecursive::wrapping(Val { x: 1 });
    let s = format!("{:?}", r);
    assert!(s.starts_with("RcRecursive(0x"), "got: {s}");
}

#[test]
fn debug_arc_recursive() {
    let r = ArcRecursive::wrapping(Val { x: 1 });
    let s = format!("{:?}", r);
    assert!(s.starts_with("ArcRecursive(0x"), "got: {s}");
}

#[test]
fn debug_rc_recursion_alive() {
    let r = RcRecursive::wrapping(Val { x: 1 });
    let rec = RcRecursion::from(&r);
    let s = format!("{:?}", rec);
    assert!(s.contains("upgrade="), "got: {s}");
}

#[test]
fn debug_rc_recursion_dangling() {
    let rec = {
        let r = RcRecursive::wrapping(Val { x: 0 });
        RcRecursion::from(&r)
    };
    let s = format!("{:?}", rec);
    assert!(s.contains("dangling"), "got: {s}");
}

#[test]
fn debug_arc_recursion_alive() {
    let r = ArcRecursive::wrapping(Val { x: 1 });
    let rec = ArcRecursion::from(&r);
    let s = format!("{:?}", rec);
    assert!(s.contains("upgrade="), "got: {s}");
}

#[test]
fn debug_arc_recursion_dangling() {
    let rec = {
        let r = ArcRecursive::wrapping(Val { x: 0 });
        ArcRecursion::from(&r)
    };
    let s = format!("{:?}", rec);
    assert!(s.contains("dangling"), "got: {s}");
}

// ── Default ──────────────────────────────────────────────────────────────────

#[test]
fn rc_anchor_default() {
    let a: RcAnchor<Val> = RcAnchor::default();
    assert_eq!(a.0.x, 0);
}

#[test]
fn arc_anchor_default() {
    let a: ArcAnchor<Val> = ArcAnchor::default();
    assert_eq!(a.0.x, 0);
}

#[test]
fn rc_recursive_default() {
    let r: RcRecursive<Val> = RcRecursive::default();
    assert_eq!(r.borrow().x, 0);
}

#[test]
fn arc_recursive_default() {
    let r: ArcRecursive<Val> = ArcRecursive::default();
    assert_eq!(r.lock().unwrap().as_ref().unwrap().x, 0);
}

// ── wrapping constructors ────────────────────────────────────────────────────

#[test]
fn rc_anchor_wrapping() {
    let a = RcAnchor::wrapping(Val { x: 42 });
    assert_eq!(a.0.x, 42);
}

#[test]
fn arc_anchor_wrapping() {
    let a = ArcAnchor::wrapping(Val { x: 43 });
    assert_eq!(a.0.x, 43);
}

// ── is_dangling helpers ──────────────────────────────────────────────────────

#[test]
fn rc_weak_anchor_is_dangling() {
    let rc = Rc::new(Val { x: 1 });
    let w = RcWeakAnchor::from(&rc);
    assert!(!w.is_dangling());
    drop(rc);
    assert!(w.is_dangling());
}

#[test]
fn arc_weak_anchor_is_dangling() {
    let arc = Arc::new(Val { x: 1 });
    let w = ArcWeakAnchor::from(&arc);
    assert!(!w.is_dangling());
    drop(arc);
    assert!(w.is_dangling());
}

#[test]
fn rc_recursion_is_dangling() {
    let r = RcRecursive::wrapping(Val { x: 1 });
    let rec = RcRecursion::from(&r);
    assert!(!rec.is_dangling());
    drop(r);
    assert!(rec.is_dangling());
}

#[test]
fn arc_recursion_is_dangling() {
    let r = ArcRecursive::wrapping(Val { x: 1 });
    let rec = ArcRecursion::from(&r);
    assert!(!rec.is_dangling());
    drop(r);
    assert!(rec.is_dangling());
}

// ── Deserialize: RcAnchor without anchor context (no alias) ─────────────────

#[test]
fn rc_anchor_deserialize_no_anchor_context() {
    // When there is no anchor in the YAML, RcAnchor still deserializes normally.
    #[derive(Deserialize)]
    struct Doc {
        a: RcAnchor<Val>,
    }
    let y = "a:\n  x: 99\n";
    let doc: Doc = serde_saphyr::from_str(y).expect("plain RcAnchor deserialize");
    assert_eq!(doc.a.0.x, 99);
}

#[test]
fn arc_anchor_deserialize_no_anchor_context() {
    #[derive(Deserialize)]
    struct Doc {
        a: ArcAnchor<Val>,
    }
    let y = "a:\n  x: 77\n";
    let doc: Doc = serde_saphyr::from_str(y).expect("plain ArcAnchor deserialize");
    assert_eq!(doc.a.0.x, 77);
}

// ── Deserialize: RcRecursive without anchor context ──────────────────────────

#[test]
fn rc_recursive_deserialize_no_anchor_context() {
    #[derive(Deserialize, Serialize)]
    struct Doc {
        a: RcRecursive<Val>,
    }
    let y = "a:\n  x: 55\n";
    let doc: Doc = serde_saphyr::from_str(y).expect("plain RcRecursive deserialize");
    assert_eq!(doc.a.borrow().x, 55);
}

#[test]
fn arc_recursive_deserialize_no_anchor_context() {
    #[derive(Deserialize, Serialize)]
    struct Doc {
        a: ArcRecursive<Val>,
    }
    let y = "a:\n  x: 33\n";
    let doc: Doc = serde_saphyr::from_str(y).expect("plain ArcRecursive deserialize");
    assert_eq!(doc.a.lock().unwrap().as_ref().unwrap().x, 33);
}

// ── Deserialize: error paths ─────────────────────────────────────────────────

#[test]
fn rc_weak_anchor_non_alias_errors() {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Doc {
        strong: RcAnchor<Val>,
        weak: RcWeakAnchor<Val>,
    }
    // weak is not an alias → should error
    let y = indoc! {r#"
        strong: &a1
          x: 1
        weak:
          x: 2
    "#};
    let res: Result<Doc, _> = serde_saphyr::from_str(y);
    assert!(res.is_err(), "expected error for non-alias RcWeakAnchor");
}

#[test]
fn arc_weak_anchor_before_strong_errors() {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Doc {
        weak: ArcWeakAnchor<Val>,
        strong: ArcAnchor<Val>,
    }
    let y = indoc! {r#"
        weak: *a1
        strong: &a1
          x: 1
    "#};
    let res: Result<Doc, _> = serde_saphyr::from_str(y);
    assert!(res.is_err(), "expected error when weak alias before strong");
}

// ── RcRecursion / ArcRecursion deserialization (via YAML alias) ──────────────

#[test]
fn rc_recursion_deserialize_via_alias() {
    #[derive(Deserialize, Serialize)]
    struct Node {
        val: i32,
        next: RcRecursion<Node>,
    }
    #[derive(Deserialize, Serialize)]
    struct Doc {
        root: RcRecursive<Node>,
    }
    let y = indoc! {r#"
        root: &r
          val: 1
          next: *r
    "#};
    let doc: Doc = serde_saphyr::from_str(y).expect("RcRecursion via alias");
    let root = doc.root.borrow();
    assert_eq!(root.val, 1);
    let next = root.next.upgrade().expect("next alive");
    assert_eq!(next.borrow().val, 1);
}

#[test]
fn arc_recursion_deserialize_via_alias() {
    #[derive(Deserialize, Serialize)]
    struct Node {
        val: i32,
        next: ArcRecursion<Node>,
    }
    #[derive(Deserialize, Serialize)]
    struct Doc {
        root: ArcRecursive<Node>,
    }
    let y = indoc! {r#"
        root: &r
          val: 2
          next: *r
    "#};
    let doc: Doc = serde_saphyr::from_str(y).expect("ArcRecursion via alias");
    let guard = doc.root.lock().unwrap();
    let root = guard.as_ref().unwrap();
    assert_eq!(root.val, 2);
    let next = root.next.upgrade().expect("next alive");
    drop(guard);
    let next_guard = next.lock().unwrap();
    assert_eq!(next_guard.as_ref().unwrap().val, 2);
}

// ── RcRecursive / ArcRecursive with alias (already-stored path) ──────────────

#[test]
fn rc_recursive_alias_reuses_existing() {
    // Two fields both pointing to the same RcRecursive anchor
    #[derive(Deserialize)]
    struct Doc {
        a: RcRecursive<Val>,
        b: RcRecursive<Val>,
    }
    let y = indoc! {r#"
        a: &v
          x: 10
        b: *v
    "#};
    let doc: Doc = serde_saphyr::from_str(y).expect("RcRecursive alias reuse");
    assert_eq!(doc.a.borrow().x, 10);
    assert_eq!(doc.b.borrow().x, 10);
    assert!(Rc::ptr_eq(&doc.a.0, &doc.b.0));
}

#[test]
fn arc_recursive_alias_reuses_existing() {
    #[derive(Deserialize)]
    struct Doc {
        a: ArcRecursive<Val>,
        b: ArcRecursive<Val>,
    }
    let y = indoc! {r#"
        a: &v
          x: 20
        b: *v
    "#};
    let doc: Doc = serde_saphyr::from_str(y).expect("ArcRecursive alias reuse");
    assert_eq!(doc.a.lock().unwrap().as_ref().unwrap().x, 20);
    assert_eq!(doc.b.lock().unwrap().as_ref().unwrap().x, 20);
    assert!(Arc::ptr_eq(&doc.a.0, &doc.b.0));
}

// ── RcRecursion::with / ArcRecursion::with ───────────────────────────────────

#[test]
fn rc_recursion_with_alive() {
    let r = RcRecursive::wrapping(Val { x: 99 });
    let rec = RcRecursion::from(&r);
    let result = rec.with(|v| v.x);
    assert_eq!(result, Some(99));
}

#[test]
fn rc_recursion_with_dangling() {
    let rec = {
        let r = RcRecursive::wrapping(Val { x: 0 });
        RcRecursion::from(&r)
    };
    assert_eq!(rec.with(|v| v.x), None);
}

#[test]
fn arc_recursion_with_alive() {
    let r = ArcRecursive::wrapping(Val { x: 88 });
    let rec = ArcRecursion::from(&r);
    let result = rec.with(|v| v.x);
    assert_eq!(result, Some(88));
}

#[test]
fn arc_recursion_with_dangling() {
    let rec = {
        let r = ArcRecursive::wrapping(Val { x: 0 });
        ArcRecursion::from(&r)
    };
    assert_eq!(rec.with(|v| v.x), None);
}
