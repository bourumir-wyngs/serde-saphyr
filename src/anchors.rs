use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Arc, Weak as ArcWeak};

/// A wrapper around [`Rc<T>`] that opt-ins a field for **anchor emission** (e.g. serialization by reference).
///
/// This type behaves like a normal [`Rc<T>`] but signals that the value
/// should be treated as an *anchorable* reference — for instance,
/// when serializing graphs or shared structures where pointer identity matters.
///
/// # Examples
///
/// ```
/// use std::rc::Rc;
/// use serde_saphyr::RcAnchor;
///
/// // Create from an existing Rc
/// let rc = Rc::new(String::from("Hello"));
/// let anchor1 = RcAnchor::from(rc.clone());
///
/// // Or directly from a value (Rc::new is called internally)
/// let anchor2: RcAnchor<String> = RcAnchor::from(Rc::new(String::from("World")));
///
/// assert_eq!(*anchor1.0, "Hello");
/// assert_eq!(*anchor2.0, "World");
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct RcAnchor<T: ?Sized>(pub Rc<T>);

/// A wrapper around [`Arc<T>`] that opt-ins a field for **anchor emission** (e.g. serialization by reference).
///
/// It behaves exactly like an [`Arc<T>`] but explicitly marks shared ownership
/// as an *anchor* for reference tracking or cross-object linking.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use serde_saphyr::ArcAnchor;
///
/// // Create from an existing Arc
/// let arc = Arc::new(String::from("Shared"));
/// let anchor1 = ArcAnchor::from(arc.clone());
///
/// // Or create directly from a value
/// let anchor2: ArcAnchor<String> = ArcAnchor::from(Arc::new(String::from("Data")));
///
/// assert_eq!(*anchor1.0, "Shared");
/// assert_eq!(*anchor2.0, "Data");
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct ArcAnchor<T: ?Sized>(pub Arc<T>);

/// A wrapper around [`Weak<T>`] (from [`std::rc`]) that opt-ins for **anchor emission**.
///
/// When serialized, if the weak reference is **dangling** (i.e., the value was dropped),
/// it emits `null` to indicate that the target no longer exists.
/// Provides convenience methods like [`upgrade`](Self::upgrade) and [`is_dangling`](Self::is_dangling).
///
/// > **Note on deserialization:** `null` deserializes back into a dangling weak (`Weak::new()`).
/// > Non-`null` cannot be safely reconstructed into a `Weak` without a shared registry; we reject it.
/// > Ask if you want an ID/registry-based scheme to restore sharing.
///
/// # Examples
///
/// ```
/// use std::rc::Rc;
/// use serde_saphyr::{RcAnchor, RcWeakAnchor};
///
/// let rc_anchor = RcAnchor::from(Rc::new(String::from("Persistent")));
///
/// // Create a weak anchor from a strong reference
/// let weak_anchor = RcWeakAnchor::from(&rc_anchor.0);
///
/// assert!(weak_anchor.upgrade().is_some());
/// drop(rc_anchor);
/// assert!(weak_anchor.upgrade().is_none());
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct RcWeakAnchor<T: ?Sized>(pub RcWeak<T>);

/// A wrapper around [`Weak<T>`] (from [`std::sync`]) that opt-ins for **anchor emission**.
///
/// This variant is thread-safe and uses [`Arc`] / [`Weak`] instead of [`Rc`].
/// If the weak reference is **dangling**, it serializes as `null`.
///
/// > **Deserialization note:** `null` → dangling weak. Non-`null` is rejected unless a registry is used.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use serde_saphyr::{ArcAnchor, ArcWeakAnchor};
///
/// let arc_anchor = ArcAnchor::from(Arc::new(String::from("Thread-safe")));
///
/// // Create a weak anchor from the strong reference
/// let weak_anchor = ArcWeakAnchor::from(&arc_anchor.0);
///
/// assert!(weak_anchor.upgrade().is_some());
/// drop(arc_anchor);
/// assert!(weak_anchor.upgrade().is_none());
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct ArcWeakAnchor<T: ?Sized>(pub ArcWeak<T>);

// ===== From conversions (strong -> anchor) =====

impl<T: ?Sized> From<Rc<T>> for RcAnchor<T> {
    #[inline]
    fn from(rc: Rc<T>) -> Self { RcAnchor(rc) }
}
impl<T: ?Sized> From<Arc<T>> for ArcAnchor<T> {
    #[inline]
    fn from(arc: Arc<T>) -> Self { ArcAnchor(arc) }
}

// ===== From conversions (strong -> weak anchor) =====

impl<T: ?Sized> From<&Rc<T>> for RcWeakAnchor<T> {
    #[inline]
    fn from(rc: &Rc<T>) -> Self { RcWeakAnchor(Rc::downgrade(rc)) }
}
impl<T: ?Sized> From<Rc<T>> for RcWeakAnchor<T> {
    #[inline]
    fn from(rc: Rc<T>) -> Self { RcWeakAnchor(Rc::downgrade(&rc)) }
}
impl<T: ?Sized> From<&RcAnchor<T>> for RcWeakAnchor<T> {
    #[inline]
    fn from(rca: &RcAnchor<T>) -> Self { RcWeakAnchor(Rc::downgrade(&rca.0)) }
}
impl<T: ?Sized> From<&Arc<T>> for ArcWeakAnchor<T> {
    #[inline]
    fn from(arc: &Arc<T>) -> Self { ArcWeakAnchor(Arc::downgrade(arc)) }
}
impl<T: ?Sized> From<Arc<T>> for ArcWeakAnchor<T> {
    #[inline]
    fn from(arc: Arc<T>) -> Self { ArcWeakAnchor(Arc::downgrade(&arc)) }
}
impl<T: ?Sized> From<&ArcAnchor<T>> for ArcWeakAnchor<T> {
    #[inline]
    fn from(ara: &ArcAnchor<T>) -> Self { ArcWeakAnchor(Arc::downgrade(&ara.0)) }
}

// ===== Ergonomics: Deref / AsRef / Borrow / Into =====

impl<T: ?Sized> Deref for RcAnchor<T> {
    type Target = Rc<T>;
    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<T: ?Sized> Deref for ArcAnchor<T> {
    type Target = Arc<T>;
    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<T: ?Sized> AsRef<Rc<T>> for RcAnchor<T> {
    #[inline]
    fn as_ref(&self) -> &Rc<T> { &self.0 }
}
impl<T: ?Sized> AsRef<Arc<T>> for ArcAnchor<T> {
    #[inline]
    fn as_ref(&self) -> &Arc<T> { &self.0 }
}
impl<T: ?Sized> Borrow<Rc<T>> for RcAnchor<T> {
    #[inline]
    fn borrow(&self) -> &Rc<T> { &self.0 }
}
impl<T: ?Sized> Borrow<Arc<T>> for ArcAnchor<T> {
    #[inline]
    fn borrow(&self) -> &Arc<T> { &self.0 }
}
impl<T: ?Sized> From<RcAnchor<T>> for Rc<T> {
    #[inline]
    fn from(a: RcAnchor<T>) -> Rc<T> { a.0 }
}
impl<T: ?Sized> From<ArcAnchor<T>> for Arc<T> {
    #[inline]
    fn from(a: ArcAnchor<T>) -> Arc<T> { a.0 }
}

// ===== Weak helpers =====

impl<T: ?Sized> RcWeakAnchor<T> {
    /// Try to upgrade the weak reference to [`Rc<T>`].
    /// Returns [`None`] if the value has been dropped.
    #[inline]
    pub fn upgrade(&self) -> Option<Rc<T>> { self.0.upgrade() }

    /// Returns `true` if the underlying value has been dropped (no strong refs remain).
    #[inline]
    pub fn is_dangling(&self) -> bool { self.0.strong_count() == 0 }
}
impl<T: ?Sized> ArcWeakAnchor<T> {
    /// Try to upgrade the weak reference to [`Arc<T>`].
    /// Returns [`None`] if the value has been dropped.
    #[inline]
    pub fn upgrade(&self) -> Option<Arc<T>> { self.0.upgrade() }

    /// Returns `true` if the underlying value has been dropped (no strong refs remain).
    #[inline]
    pub fn is_dangling(&self) -> bool { self.0.strong_count() == 0 }
}

// ===== Pointer-equality PartialEq/Eq =====

impl<T: ?Sized> PartialEq for RcAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) }
}
impl<T: ?Sized> Eq for RcAnchor<T> {}

impl<T: ?Sized> PartialEq for ArcAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}
impl<T: ?Sized> Eq for ArcAnchor<T> {}

impl<T: ?Sized> PartialEq for RcWeakAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0.upgrade(), other.0.upgrade()) {
            (Some(a), Some(b)) => Rc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}
impl<T: ?Sized> Eq for RcWeakAnchor<T> {}

impl<T: ?Sized> PartialEq for ArcWeakAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0.upgrade(), other.0.upgrade()) {
            (Some(a), Some(b)) => Arc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}
impl<T: ?Sized> Eq for ArcWeakAnchor<T> {}

// ===== Debug =====

impl<T: ?Sized> fmt::Debug for RcAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RcAnchor({:p})", Rc::as_ptr(&self.0))
    }
}
impl<T: ?Sized> fmt::Debug for ArcAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArcAnchor({:p})", Arc::as_ptr(&self.0))
    }
}
impl<T: ?Sized> fmt::Debug for RcWeakAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(rc) = self.0.upgrade() {
            write!(f, "RcWeakAnchor(upgrade={:p})", Rc::as_ptr(&rc))
        } else {
            write!(f, "RcWeakAnchor(dangling)")
        }
    }
}
impl<T: ?Sized> fmt::Debug for ArcWeakAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(arc) = self.0.upgrade() {
            write!(f, "ArcWeakAnchor(upgrade={:p})", Arc::as_ptr(&arc))
        } else {
            write!(f, "ArcWeakAnchor(dangling)")
        }
    }
}

// ===== Default =====

impl<T: Default> Default for RcAnchor<T> {
    #[inline]
    fn default() -> Self { RcAnchor(Rc::new(T::default())) }
}
impl<T: Default> Default for ArcAnchor<T> {
    fn default() -> Self { ArcAnchor(Arc::new(T::default())) }
}

// -------------------------------
// Deserialize impls
// -------------------------------
impl<'de, T> serde::de::Deserialize<'de> for RcAnchor<T>
where
    T: serde::de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let inner = T::deserialize(deserializer)?;
        Ok(RcAnchor(Rc::new(inner)))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for ArcAnchor<T>
where
    T: serde::de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let inner = T::deserialize(deserializer)?;
        Ok(ArcAnchor(Arc::new(inner)))
    }
}

