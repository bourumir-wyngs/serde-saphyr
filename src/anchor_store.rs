use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnchorKind {
    Rc,
    Arc,
}

#[derive(Default)]
struct AnchorStore {
    rc: HashMap<usize, Rc<dyn Any>>,
    arc: HashMap<usize, Arc<dyn Any + Send + Sync>>,
}

#[derive(Default)]
struct AnchorState {
    stack: Vec<(AnchorKind, usize)>,
    store: AnchorStore,
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
    });
}

pub(crate) fn with_anchor_context<R>(
    kind: AnchorKind,
    anchor: Option<usize>,
    f: impl FnOnce() -> R,
) -> R {
    if let Some(id) = anchor {
        STATE.with(|state| state.borrow_mut().stack.push((kind, id)));
        let guard = Guard;
        let result = f();
        drop(guard);
        result
    } else {
        f()
    }
}

struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.stack.pop();
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
