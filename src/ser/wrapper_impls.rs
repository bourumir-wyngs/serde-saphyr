use serde_core::ser::{self, Serialize, SerializeTupleStruct, Serializer};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::{
    ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor, Commented, FlowMap, FlowSeq, Quoted,
    RcAnchor, RcRecursion, RcRecursive, RcWeakAnchor, SpaceAfter,
};

use super::{
    NAME_FLOW_MAP, NAME_FLOW_SEQ, NAME_QUOTED, NAME_SPACE_AFTER, NAME_TUPLE_ANCHOR,
    NAME_TUPLE_COMMENTED, NAME_TUPLE_WEAK,
};

// ------------------------------------------------------------
// Internal wrappers -> shape the stream Serde produces so our
// serializer can intercept them in a single pass.
// ------------------------------------------------------------

// Recursive weak payloads: defer locking/borrowing until actual value serialization.
struct RcRecursivePayload<'a, T>(&'a Rc<RefCell<Option<T>>>);
struct ArcRecursivePayload<'a, T>(&'a Arc<Mutex<Option<T>>>);

impl<T: Serialize> Serialize for RcRecursivePayload<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let borrowed = self.0.borrow();
        if let Some(value) = borrowed.as_ref() {
            value.serialize(s)
        } else {
            s.serialize_unit()
        }
    }
}

impl<T: Serialize> Serialize for ArcRecursivePayload<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let guard = self
            .0
            .lock()
            .map_err(|_| ser::Error::custom("recursive Arc anchor mutex poisoned"))?;
        if let Some(value) = guard.as_ref() {
            value.serialize(s)
        } else {
            s.serialize_unit()
        }
    }
}

// Top-level newtype wrappers for strong/weak simply wrap the real payloads.
impl<T: Serialize> Serialize for RcAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // delegate to tuple-struct the serializer knows how to intercept
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Rc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&*self.0)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Arc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&*self.0)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for RcRecursive<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Rc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        let borrowed = self.0.borrow();
        let value = borrowed
            .as_ref()
            .ok_or_else(|| ser::Error::custom("recursive Rc anchor not initialized"))?;
        ts.serialize_field(value)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcRecursive<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Arc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        let guard = self
            .0
            .lock()
            .map_err(|_| ser::Error::custom("recursive Arc anchor mutex poisoned"))?;
        let value = guard
            .as_ref()
            .ok_or_else(|| ser::Error::custom("recursive Arc anchor not initialized"))?;
        ts.serialize_field(value)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for RcWeakAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(rc) = up {
            ts.serialize_field(&*rc)?;
        } else {
            ts.serialize_field(&())?; // ignored by our serializer
        }
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcWeakAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(arc) = up {
            ts.serialize_field(&*arc)?;
        } else {
            ts.serialize_field(&())?;
        }
        ts.end()
    }
}
impl<T: Serialize> Serialize for RcRecursion<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(rc) = up {
            ts.serialize_field(&RcRecursivePayload(&rc))?;
        } else {
            ts.serialize_field(&())?;
        }
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcRecursion<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(arc) = up {
            ts.serialize_field(&ArcRecursivePayload(&arc))?;
        } else {
            ts.serialize_field(&())?;
        }
        ts.end()
    }
}

// Hints for flow / block strings.
impl<T: Serialize> Serialize for FlowSeq<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FLOW_SEQ, &self.0)
    }
}
impl<T: Serialize> Serialize for FlowMap<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FLOW_MAP, &self.0)
    }
}
impl<T: Serialize> Serialize for SpaceAfter<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_SPACE_AFTER, &self.0)
    }
}
impl<T: AsRef<str>> Serialize for Quoted<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_QUOTED, self.0.as_ref())
    }
}

impl<T: Serialize> Serialize for Commented<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // Represent as a special tuple-struct with two fields: (comment, value)
        // so the serializer can stage the comment before serializing the value.
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_COMMENTED, 2)?;
        ts.serialize_field(&self.1)?; // comment first
        ts.serialize_field(&self.0)?; // then value
        ts.end()
    }
}
