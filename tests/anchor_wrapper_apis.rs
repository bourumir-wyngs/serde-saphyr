#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde_saphyr::{
    ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor, RcAnchor, RcRecursion, RcRecursive,
    RcWeakAnchor,
};
use std::borrow::Borrow;
use std::rc::Rc;
use std::sync::Arc;

mod anchors_tests {
    use super::*;

    #[test]
    fn arc_anchor_wrapping_deref_asref_borrow_from_into() {
        let arc = Arc::new("hello".to_string());
        let anch1: ArcAnchor<String> = ArcAnchor::wrapping("world".to_string());
        let anch2: ArcAnchor<String> = arc.clone().into();
        assert_eq!(&**anch1, "world");
        assert_eq!(&**anch2, "hello");
        let _deref: &Arc<String> = &anch2;
        let _asref: &Arc<String> = anch2.as_ref();
        let _borrow: &Arc<String> = Borrow::borrow(&anch2);
        let back: Arc<String> = anch2.into();
        assert!(Arc::ptr_eq(&back, &arc));
    }

    #[test]
    fn arc_anchor_default_debug() {
        let anch: ArcAnchor<()> = ArcAnchor::default();
        let _dbg = format!("{:?}", anch);
    }

    #[test]
    fn rc_anchor_default_debug() {
        let anch: RcAnchor<()> = RcAnchor::default();
        let _dbg = format!("{:?}", anch);
    }

    #[test]
    fn arc_weak_anchor_from_strong_anchor() {
        let strong = Arc::new("hi".to_string());
        let weak1: ArcWeakAnchor<String> = (&strong).into();
        let anch = ArcAnchor::wrapping("hi".to_string());
        let weak3: ArcWeakAnchor<String> = (&anch).into();
        drop(strong);
        drop(anch);
        assert!(weak1.upgrade().is_none());
        assert!(weak1.is_dangling());
        assert!(weak3.upgrade().is_none());
        assert!(weak3.is_dangling());
    }

    #[test]
    fn rc_weak_anchor_from_strong_anchor() {
        let strong = Rc::new("hi".to_string());
        let weak1: RcWeakAnchor<String> = (&strong).into();
        let anch = RcAnchor::wrapping("hi".to_string());
        let weak3: RcWeakAnchor<String> = (&anch).into();
        drop(strong);
        drop(anch);
        assert!(weak1.upgrade().is_none());
        assert!(weak1.is_dangling());
        assert!(weak3.upgrade().is_none());
        assert!(weak3.is_dangling());
    }

    #[test]
    fn recursion_weak_from_strong_dangling() {
        let rec_strong_rc = RcRecursive::wrapping("rc".to_string());
        let rec_weak_rc: RcRecursion<String> = (&rec_strong_rc).into();
        drop(rec_strong_rc);
        assert!(rec_weak_rc.is_dangling());
        assert!(rec_weak_rc.upgrade().is_none());

        let rec_strong_arc = ArcRecursive::wrapping("arc".to_string());
        let rec_weak_arc: ArcRecursion<String> = (&rec_strong_arc).into();
        drop(rec_strong_arc);
        assert!(rec_weak_arc.is_dangling());
        assert!(rec_weak_arc.upgrade().is_none());
    }
}
