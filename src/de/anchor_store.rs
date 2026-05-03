use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AnchorKind {
    Rc,
    Arc,
    RcRecursive,
    ArcRecursive,
}

#[derive(Default)]
struct AnchorStore {
    rc: HashMap<usize, Rc<dyn Any>>,
    arc: HashMap<usize, Arc<dyn Any + Send + Sync>>,
    rc_recursive: HashMap<usize, Rc<dyn Any>>,
    arc_recursive: HashMap<usize, Arc<dyn Any + Send + Sync>>,
}

#[derive(Default)]
struct AnchorState {
    stack: Vec<AnchorFrame>,
    store: AnchorStore,
    in_progress: HashMap<(AnchorKind, usize), usize>,
}

#[derive(Clone, Copy)]
struct AnchorFrame {
    kind: AnchorKind,
    id: usize,
    claimed: bool,
}

thread_local! {
    static STATE: RefCell<AnchorState> = RefCell::new(AnchorState::default());
}

pub(crate) fn reset() {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.stack.clear();
        s.store.rc.clear();
        s.store.arc.clear();
        s.store.rc_recursive.clear();
        s.store.arc_recursive.clear();
        s.in_progress.clear();
    });
}

pub(crate) fn with_anchor_context<R>(
    kind: AnchorKind,
    anchor: Option<usize>,
    f: impl FnOnce() -> R,
) -> R {
    if let Some(id) = anchor {
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.stack.push(AnchorFrame {
                kind,
                id,
                claimed: false,
            });
            *s.in_progress.entry((kind, id)).or_insert(0) += 1;
        });
        let guard = Guard { kind, id };
        let result = f();
        drop(guard);
        result
    } else {
        f()
    }
}

struct Guard {
    kind: AnchorKind,
    id: usize,
}

impl Drop for Guard {
    fn drop(&mut self) {
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            let popped = s.stack.pop();
            debug_assert!(
                matches!(
                    popped,
                    Some(AnchorFrame { kind, id, .. })
                        if kind == self.kind && id == self.id
                ),
                "anchor context stack corrupted"
            );

            if let Some(count) = s.in_progress.get_mut(&(self.kind, self.id)) {
                if *count > 1 {
                    *count -= 1;
                } else {
                    s.in_progress.remove(&(self.kind, self.id));
                }
            }
        });
    }
}

/// Return the innermost active anchor id for `kind`, but only if that frame has not
/// already been claimed by the wrapper whose YAML node introduced it.
///
/// This is intentionally stricter than searching the whole stack. Searching outward
/// lets nested `RcAnchor` / `ArcAnchor` values accidentally inherit an enclosing
/// anchor, which is what caused issue #106.
fn current_anchor_id(kind: AnchorKind) -> Option<usize> {
    STATE.with(|state| {
        let s = state.borrow();
        let frame = s.stack.last()?;

        if frame.kind == kind && !frame.claimed {
            Some(frame.id)
        } else {
            None
        }
    })
}

/// Claim the innermost active anchor id for `kind`.
///
/// Strong anchor wrappers call this so the frame belongs to exactly one wrapper.
/// Nested deserialization performed while the wrapper's value is being read can no
/// longer reuse the enclosing anchor id by accident.
fn claim_anchor_id(kind: AnchorKind) -> Option<usize> {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        let frame = s.stack.last_mut()?;

        if frame.kind == kind && !frame.claimed {
            frame.claimed = true;
            Some(frame.id)
        } else {
            None
        }
    })
}

pub(crate) fn current_rc_anchor() -> Option<usize> {
    current_anchor_id(AnchorKind::Rc)
}

pub(crate) fn current_arc_anchor() -> Option<usize> {
    current_anchor_id(AnchorKind::Arc)
}

pub(crate) fn current_rc_recursive_anchor() -> Option<usize> {
    current_anchor_id(AnchorKind::RcRecursive)
}

pub(crate) fn current_arc_recursive_anchor() -> Option<usize> {
    current_anchor_id(AnchorKind::ArcRecursive)
}

pub(crate) fn claim_rc_anchor() -> Option<usize> {
    claim_anchor_id(AnchorKind::Rc)
}

pub(crate) fn claim_arc_anchor() -> Option<usize> {
    claim_anchor_id(AnchorKind::Arc)
}

pub(crate) fn claim_rc_recursive_anchor() -> Option<usize> {
    claim_anchor_id(AnchorKind::RcRecursive)
}

pub(crate) fn claim_arc_recursive_anchor() -> Option<usize> {
    claim_anchor_id(AnchorKind::ArcRecursive)
}

fn anchor_reentrant(kind: AnchorKind, id: usize) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .in_progress
            .get(&(kind, id))
            .copied()
            .unwrap_or(0)
            > 1
    })
}

pub(crate) fn rc_anchor_reentrant(id: usize) -> bool {
    anchor_reentrant(AnchorKind::Rc, id)
}

pub(crate) fn arc_anchor_reentrant(id: usize) -> bool {
    anchor_reentrant(AnchorKind::Arc, id)
}

pub(crate) fn rc_recursive_reentrant(id: usize) -> bool {
    anchor_reentrant(AnchorKind::RcRecursive, id)
}

pub(crate) fn arc_recursive_reentrant(id: usize) -> bool {
    anchor_reentrant(AnchorKind::ArcRecursive, id)
}

pub(crate) fn recursive_anchor_in_progress(id: usize) -> bool {
    STATE.with(|state| {
        let s = state.borrow();
        s.in_progress.contains_key(&(AnchorKind::RcRecursive, id))
            || s.in_progress.contains_key(&(AnchorKind::ArcRecursive, id))
    })
}

pub(crate) fn store_rc<T: Any>(id: usize, rc: Rc<T>) {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.store.rc.insert(id, rc);
    });
}

pub(crate) fn store_arc<T: Any + Send + Sync>(id: usize, arc: Arc<T>) {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.store.arc.insert(id, arc);
    });
}

pub(crate) fn store_rc_recursive<T: Any>(id: usize, rc: Rc<T>) {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.store.rc_recursive.insert(id, rc);
    });
}

pub(crate) fn store_arc_recursive<T: Any + Send + Sync>(id: usize, arc: Arc<T>) {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.store.arc_recursive.insert(id, arc);
    });
}

pub(crate) fn get_rc<T: Any>(id: usize) -> Result<Option<Rc<T>>, String> {
    STATE.with(|state| {
        let s = state.borrow();
        if let Some(existing) = s.store.rc.get(&id) {
            let cloned = existing.clone();
            match cloned.downcast::<T>() {
                Ok(rc_t) => Ok(Some(rc_t)),
                Err(_) => Err(format!("anchor id {} reused with incompatible Rc type", id)),
            }
        } else {
            Ok(None)
        }
    })
}

pub(crate) fn get_arc<T: Any + Send + Sync>(id: usize) -> Result<Option<Arc<T>>, String> {
    STATE.with(|state| {
        let s = state.borrow();
        if let Some(existing) = s.store.arc.get(&id) {
            let cloned = existing.clone();
            match cloned.downcast::<T>() {
                Ok(arc_t) => Ok(Some(arc_t)),
                Err(_) => Err(format!(
                    "anchor id {} reused with incompatible Arc type",
                    id
                )),
            }
        } else {
            Ok(None)
        }
    })
}

pub(crate) fn get_rc_recursive<T: Any>(id: usize) -> Result<Option<Rc<T>>, String> {
    STATE.with(|state| {
        let s = state.borrow();
        if let Some(existing) = s.store.rc_recursive.get(&id) {
            let cloned = existing.clone();
            match cloned.downcast::<T>() {
                Ok(rc_t) => Ok(Some(rc_t)),
                Err(_) => Err(format!(
                    "recursive anchor id {} reused with incompatible Rc type",
                    id
                )),
            }
        } else {
            Ok(None)
        }
    })
}

pub(crate) fn get_arc_recursive<T: Any + Send + Sync>(id: usize) -> Result<Option<Arc<T>>, String> {
    STATE.with(|state| {
        let s = state.borrow();
        if let Some(existing) = s.store.arc_recursive.get(&id) {
            let cloned = existing.clone();
            match cloned.downcast::<T>() {
                Ok(arc_t) => Ok(Some(arc_t)),
                Err(_) => Err(format!(
                    "recursive anchor id {} reused with incompatible Arc type",
                    id
                )),
            }
        } else {
            Ok(None)
        }
    })
}

pub(crate) fn with_document_scope<R>(f: impl FnOnce() -> R) -> R {
    reset();
    struct ResetGuard;
    impl Drop for ResetGuard {
        fn drop(&mut self) {
            reset();
        }
    }
    let guard = ResetGuard;
    let result = f();
    drop(guard);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Mutex;

    struct IsolatedState;

    impl Drop for IsolatedState {
        fn drop(&mut self) {
            reset();
        }
    }

    fn isolated_state() -> IsolatedState {
        reset();
        IsolatedState
    }

    #[test]
    fn context_without_anchor_does_not_touch_state() {
        let _state = isolated_state();

        let value = with_anchor_context(AnchorKind::Rc, None, || {
            assert_eq!(current_rc_anchor(), None);
            assert!(!rc_anchor_reentrant(1));
            42
        });

        assert_eq!(value, 42);
        assert_eq!(current_rc_anchor(), None);
        assert!(!rc_anchor_reentrant(1));
    }

    #[test]
    fn current_anchor_is_only_innermost_unclaimed_matching_frame() {
        let _state = isolated_state();

        with_anchor_context(AnchorKind::Rc, Some(7), || {
            assert_eq!(current_rc_anchor(), Some(7));
            assert_eq!(current_arc_anchor(), None);

            with_anchor_context(AnchorKind::Arc, Some(8), || {
                assert_eq!(current_rc_anchor(), None);
                assert_eq!(current_arc_anchor(), Some(8));
                assert_eq!(claim_arc_anchor(), Some(8));
                assert_eq!(current_arc_anchor(), None);
                assert_eq!(claim_arc_anchor(), None);
            });

            assert_eq!(current_rc_anchor(), Some(7));
            assert_eq!(claim_rc_anchor(), Some(7));
            assert_eq!(current_rc_anchor(), None);

            with_anchor_context(AnchorKind::Rc, Some(9), || {
                assert_eq!(current_rc_anchor(), Some(9));
            });

            assert_eq!(claim_rc_anchor(), None);
        });

        assert_eq!(current_rc_anchor(), None);
    }

    #[test]
    fn recursive_current_and_claim_use_distinct_anchor_kinds() {
        let _state = isolated_state();

        with_anchor_context(AnchorKind::RcRecursive, Some(3), || {
            assert_eq!(current_rc_recursive_anchor(), Some(3));
            assert_eq!(current_arc_recursive_anchor(), None);
            assert!(recursive_anchor_in_progress(3));

            with_anchor_context(AnchorKind::ArcRecursive, Some(3), || {
                assert_eq!(current_rc_recursive_anchor(), None);
                assert_eq!(current_arc_recursive_anchor(), Some(3));
                assert!(recursive_anchor_in_progress(3));
                assert_eq!(claim_arc_recursive_anchor(), Some(3));
                assert_eq!(current_arc_recursive_anchor(), None);
                assert_eq!(claim_arc_recursive_anchor(), None);
            });

            assert_eq!(current_rc_recursive_anchor(), Some(3));
            assert_eq!(claim_rc_recursive_anchor(), Some(3));
            assert_eq!(current_rc_recursive_anchor(), None);
            assert_eq!(claim_rc_recursive_anchor(), None);
        });

        assert!(!recursive_anchor_in_progress(3));
    }

    #[test]
    fn reentrant_tracking_counts_nested_matching_kind_and_id() {
        let _state = isolated_state();

        with_anchor_context(AnchorKind::Rc, Some(5), || {
            assert!(!rc_anchor_reentrant(5));

            with_anchor_context(AnchorKind::Rc, Some(6), || {
                assert!(!rc_anchor_reentrant(5));
                assert!(!rc_anchor_reentrant(6));
            });

            with_anchor_context(AnchorKind::Rc, Some(5), || {
                assert!(rc_anchor_reentrant(5));
            });

            assert!(!rc_anchor_reentrant(5));
        });

        with_anchor_context(AnchorKind::Arc, Some(5), || {
            with_anchor_context(AnchorKind::Arc, Some(5), || {
                assert!(arc_anchor_reentrant(5));
            });
        });

        with_anchor_context(AnchorKind::RcRecursive, Some(7), || {
            with_anchor_context(AnchorKind::RcRecursive, Some(7), || {
                assert!(rc_recursive_reentrant(7));
            });
        });

        with_anchor_context(AnchorKind::ArcRecursive, Some(8), || {
            with_anchor_context(AnchorKind::ArcRecursive, Some(8), || {
                assert!(arc_recursive_reentrant(8));
            });
        });

        assert!(!rc_anchor_reentrant(5));
        assert!(!arc_anchor_reentrant(5));
        assert!(!rc_recursive_reentrant(7));
        assert!(!arc_recursive_reentrant(8));
    }

    #[test]
    fn stores_return_matching_values_and_keep_anchor_kinds_separate() {
        let _state = isolated_state();

        let rc = Rc::new("rc".to_owned());
        let arc = Arc::new("arc".to_owned());
        let rc_recursive = Rc::new(RefCell::new(Some(10_i32)));
        let arc_recursive = Arc::new(Mutex::new(Some(11_i32)));

        store_rc(1, rc.clone());
        store_arc(1, arc.clone());
        store_rc_recursive(1, rc_recursive.clone());
        store_arc_recursive(1, arc_recursive.clone());

        assert!(Rc::ptr_eq(&get_rc::<String>(1).unwrap().unwrap(), &rc));
        assert!(Arc::ptr_eq(&get_arc::<String>(1).unwrap().unwrap(), &arc));
        assert!(Rc::ptr_eq(
            &get_rc_recursive::<RefCell<Option<i32>>>(1)
                .unwrap()
                .unwrap(),
            &rc_recursive
        ));
        assert!(Arc::ptr_eq(
            &get_arc_recursive::<Mutex<Option<i32>>>(1).unwrap().unwrap(),
            &arc_recursive
        ));

        assert!(get_rc::<String>(2).unwrap().is_none());
        assert!(get_arc::<String>(2).unwrap().is_none());
        assert!(
            get_rc_recursive::<RefCell<Option<i32>>>(2)
                .unwrap()
                .is_none()
        );
        assert!(
            get_arc_recursive::<Mutex<Option<i32>>>(2)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn stores_report_incompatible_types() {
        let _state = isolated_state();

        store_rc(1, Rc::new(1_u32));
        store_arc(2, Arc::new(2_u32));
        store_rc_recursive(3, Rc::new(RefCell::new(Some(3_u32))));
        store_arc_recursive(4, Arc::new(Mutex::new(Some(4_u32))));

        assert_eq!(
            get_rc::<String>(1).unwrap_err(),
            "anchor id 1 reused with incompatible Rc type"
        );
        assert_eq!(
            get_arc::<String>(2).unwrap_err(),
            "anchor id 2 reused with incompatible Arc type"
        );
        assert_eq!(
            get_rc_recursive::<RefCell<Option<String>>>(3).unwrap_err(),
            "recursive anchor id 3 reused with incompatible Rc type"
        );
        assert_eq!(
            get_arc_recursive::<Mutex<Option<String>>>(4).unwrap_err(),
            "recursive anchor id 4 reused with incompatible Arc type"
        );
    }

    #[test]
    fn reset_clears_contexts_reentrant_counts_and_stores() {
        let _state = isolated_state();

        with_anchor_context(AnchorKind::Rc, Some(1), || {
            with_anchor_context(AnchorKind::Rc, Some(1), || {
                assert!(rc_anchor_reentrant(1));
            });
        });
        store_rc(1, Rc::new("stored".to_owned()));

        assert!(get_rc::<String>(1).unwrap().is_some());
        reset();

        assert_eq!(current_rc_anchor(), None);
        assert!(!rc_anchor_reentrant(1));
        assert!(get_rc::<String>(1).unwrap().is_none());
    }

    #[test]
    fn document_scope_resets_before_and_after_success() {
        let _state = isolated_state();

        store_rc(1, Rc::new(1_i32));

        let result = with_document_scope(|| {
            assert!(get_rc::<i32>(1).unwrap().is_none());

            store_rc(2, Rc::new(2_i32));
            with_anchor_context(AnchorKind::Rc, Some(2), || {
                assert_eq!(current_rc_anchor(), Some(2));
            });

            "ok"
        });

        assert_eq!(result, "ok");
        assert_eq!(current_rc_anchor(), None);
        assert!(get_rc::<i32>(1).unwrap().is_none());
        assert!(get_rc::<i32>(2).unwrap().is_none());
    }

    #[test]
    fn document_scope_resets_after_panic() {
        let _state = isolated_state();

        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            with_document_scope(|| {
                store_rc(9, Rc::new(9_i32));

                with_anchor_context(AnchorKind::Rc, Some(9), || {
                    assert_eq!(current_rc_anchor(), Some(9));
                    panic!("intentional panic while testing anchor store cleanup");
                });
            });
        }));

        assert!(panic_result.is_err());
        assert_eq!(current_rc_anchor(), None);
        assert!(!rc_anchor_reentrant(9));
        assert!(get_rc::<i32>(9).unwrap().is_none());
    }
}
