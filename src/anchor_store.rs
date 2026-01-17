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
    stack: Vec<(AnchorKind, usize)>,
    store: AnchorStore,
    in_progress: HashMap<(AnchorKind, usize), usize>,
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
            s.stack.push((kind, id));
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
            s.stack.pop();
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

fn current_anchor_id(kind: AnchorKind) -> Option<usize> {
    STATE.with(|state| {
        state
            .borrow()
            .stack
            .iter()
            .rev()
            .find_map(|(k, id)| if *k == kind { Some(*id) } else { None })
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
