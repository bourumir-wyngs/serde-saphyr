use std::borrow::Borrow;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Arc, Mutex, Weak as ArcWeak};

use serde::de::{Error as _, Visitor};

use crate::anchor_store;

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
pub struct RcAnchor<T>(pub Rc<T>);

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
pub struct ArcAnchor<T>(pub Arc<T>);

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
pub struct RcWeakAnchor<T>(pub RcWeak<T>);

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
pub struct ArcWeakAnchor<T>(pub ArcWeak<T>);

/// The parent (origin) anchor definition that may have recursive references to it.
/// This type provides the value for the references and must be placed where the original value is defined.
/// Fields that reference this value (possibly recursively) must be wrapped in [`RcRecursion`].
/// ```rust
/// use std::cell::Ref;
/// use serde::{Deserialize, Serialize};
/// use serde_saphyr::{RcRecursion, RcRecursive};
/// #[derive(Deserialize, Serialize)]
/// struct King {
///     name: String,
///     coronator: RcRecursion<King>, // who crowned this king
/// }
///
/// #[derive(Deserialize, Serialize)]
/// struct Kingdom {
///     king: RcRecursive<King>,
/// }
///     let yaml = r#"
/// king: &root
///   name: "Aurelian I"
///   coronator: *root # this king crowned himself
/// "#;
///
/// let kingdom_data: Kingdom = serde_saphyr::from_str(yaml).unwrap();
///     let king: Ref<King> = kingdom_data.king.borrow();
///     let coronator = king
///         .coronator
///         .upgrade().expect("coronator always exists");
///     let coronator_name = &coronator.borrow().name;
///     assert_eq!(coronator_name, "Aurelian I");
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct RcRecursive<T>(pub Rc<RefCell<Option<T>>>);

/// The parent (origin) anchor definition that may have recursive references to it.
/// This type provides the value for the references and must be placed where the original value is defined.
/// Fields that reference this value (possibly recursively) must be wrapped in [`ArcRecursion`].
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use serde_saphyr::{ArcRecursion, ArcRecursive};
///
/// #[derive(Deserialize, Serialize)]
/// struct King {
///     name: String,
///     coronator: ArcRecursion<King>, // who crowned this king
/// }
///
/// #[derive(Deserialize, Serialize)]
/// struct Kingdom {
///     king: ArcRecursive<King>,
/// }
///
///     let yaml = r#"
/// king: &root
///   name: "Aurelian I"
///   coronator: *root # this king crowned himself
/// "#;
///
///     let kingdom_data: Kingdom = serde_saphyr::from_str(yaml).unwrap();
///     let coronator = {
///         let king_guard = kingdom_data.king.lock().unwrap();
///         let king = king_guard.as_ref().expect("king should be initialized");
///         king.coronator
///             .upgrade()
///             .expect("coronator should be alive")
///     };
///
///     let coronator_guard = coronator.lock().unwrap();
///     let coronator_ref = coronator_guard
///         .as_ref()
///         .expect("coronator should be initialized");
///     assert_eq!(coronator_ref.name, "Aurelian I");
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct ArcRecursive<T>(pub Arc<Mutex<Option<T>>>);

/// The possibly recursive reference to the parent anchor that must be [`RcRecursive`].
/// See [`RcRecursive`] for code example.
#[repr(transparent)]
#[derive(Clone)]
pub struct RcRecursion<T>(pub RcWeak<RefCell<Option<T>>>);

/// The possibly recursive reference to the parent anchor that must be [`ArcRecursive`], thread safe
/// It is more complex to use than [`RcRecursive`] (you need to lock it before accessing the value)
/// See [`ArcRecursive`] for code example.
#[repr(transparent)]
#[derive(Clone)]
pub struct ArcRecursion<T>(pub ArcWeak<Mutex<Option<T>>>);

// ===== From conversions (strong -> anchor) =====

impl<T> From<Rc<T>> for RcAnchor<T> {
    fn from(rc: Rc<T>) -> Self {
        RcAnchor(rc)
    }
}

impl<T> RcAnchor<T> {
    /// Create inner Rc (takes arbitrary value Rc can take)
    pub fn wrapping(x: T) -> Self {
        RcAnchor(Rc::new(x))
    }
}

impl<T> ArcAnchor<T> {
    /// Create inner Arc (takes arbitrary value Arc can take)
    pub fn wrapping(x: T) -> Self {
        ArcAnchor(Arc::new(x))
    }
}

impl<T> From<Arc<T>> for ArcAnchor<T> {
    #[inline]
    fn from(arc: Arc<T>) -> Self {
        ArcAnchor(arc)
    }
}

// ===== From conversions (strong -> weak anchor) =====

impl<T> From<&Rc<T>> for RcWeakAnchor<T> {
    #[inline]
    fn from(rc: &Rc<T>) -> Self {
        RcWeakAnchor(Rc::downgrade(rc))
    }
}
impl<T> From<Rc<T>> for RcWeakAnchor<T> {
    #[inline]
    fn from(rc: Rc<T>) -> Self {
        RcWeakAnchor(Rc::downgrade(&rc))
    }
}
impl<T> From<&RcAnchor<T>> for RcWeakAnchor<T> {
    #[inline]
    fn from(rca: &RcAnchor<T>) -> Self {
        RcWeakAnchor(Rc::downgrade(&rca.0))
    }
}
impl<T> From<&Arc<T>> for ArcWeakAnchor<T> {
    #[inline]
    fn from(arc: &Arc<T>) -> Self {
        ArcWeakAnchor(Arc::downgrade(arc))
    }
}
impl<T> From<Arc<T>> for ArcWeakAnchor<T> {
    #[inline]
    fn from(arc: Arc<T>) -> Self {
        ArcWeakAnchor(Arc::downgrade(&arc))
    }
}
impl<T> From<&ArcAnchor<T>> for ArcWeakAnchor<T> {
    #[inline]
    fn from(ara: &ArcAnchor<T>) -> Self {
        ArcWeakAnchor(Arc::downgrade(&ara.0))
    }
}

// ===== From conversions (recursive strong -> weak) =====

impl<T> From<&RcRecursive<T>> for RcRecursion<T> {
    #[inline]
    fn from(rca: &RcRecursive<T>) -> Self {
        RcRecursion(Rc::downgrade(&rca.0))
    }
}

impl<T> From<&ArcRecursive<T>> for ArcRecursion<T> {
    #[inline]
    fn from(ara: &ArcRecursive<T>) -> Self {
        ArcRecursion(Arc::downgrade(&ara.0))
    }
}

// ===== Ergonomics: Deref / AsRef / Borrow / Into =====

impl<T> Deref for RcAnchor<T> {
    type Target = Rc<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> Deref for ArcAnchor<T> {
    type Target = Arc<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> Deref for RcRecursive<T> {
    type Target = Rc<RefCell<Option<T>>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> Deref for ArcRecursive<T> {
    type Target = Arc<Mutex<Option<T>>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> AsRef<Rc<T>> for RcAnchor<T> {
    #[inline]
    fn as_ref(&self) -> &Rc<T> {
        &self.0
    }
}
impl<T> AsRef<Arc<T>> for ArcAnchor<T> {
    #[inline]
    fn as_ref(&self) -> &Arc<T> {
        &self.0
    }
}
impl<T> Borrow<Rc<T>> for RcAnchor<T> {
    #[inline]
    fn borrow(&self) -> &Rc<T> {
        &self.0
    }
}
impl<T> Borrow<Arc<T>> for ArcAnchor<T> {
    #[inline]
    fn borrow(&self) -> &Arc<T> {
        &self.0
    }
}
impl<T> From<RcAnchor<T>> for Rc<T> {
    #[inline]
    fn from(a: RcAnchor<T>) -> Rc<T> {
        a.0
    }
}
impl<T> From<ArcAnchor<T>> for Arc<T> {
    #[inline]
    fn from(a: ArcAnchor<T>) -> Arc<T> {
        a.0
    }
}

impl<T> RcRecursive<T> {
    /// Create a new recursive anchor with an initialized value.
    pub fn wrapping(x: T) -> Self {
        RcRecursive(Rc::new(RefCell::new(Some(x))))
    }

    /// Borrow the inner value
    pub fn borrow(&self) -> std::cell::Ref<'_, T> {
        let borrowed = self.0.as_ref().borrow();
        std::cell::Ref::map(borrowed, |opt: &Option<T>| {
            opt.as_ref()
                .expect("recursive Rc anchor not initialized")
        })
    }
}

impl<T> ArcRecursive<T> {
    /// Create a new recursive anchor with an initialized value.
    pub fn wrapping(x: T) -> Self {
        ArcRecursive(Arc::new(Mutex::new(Some(x))))
    }

    /// Lock the recursive anchor value so that it can be accessed safely.
    pub fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Option<T>>> {
        self.0.lock()
    }
}

// ===== Weak helpers =====

impl<T> RcWeakAnchor<T> {
    /// Try to upgrade the weak reference to [`Rc<T>`].
    /// Returns [`None`] if the value has been dropped.
    #[inline]
    pub fn upgrade(&self) -> Option<Rc<T>> {
        self.0.upgrade()
    }

    /// Returns `true` if the underlying value has been dropped (no strong refs remain).
    #[inline]
    pub fn is_dangling(&self) -> bool {
        self.0.strong_count() == 0
    }
}
impl<T> RcRecursion<T> {
    /// Try to upgrade the weak reference to [`RcRecursive<T>`].
    #[inline]
    pub fn upgrade(&self) -> Option<RcRecursive<T>> {
        self.0.upgrade().map(RcRecursive)
    }

    /// Access the recursive value in one step, if it is still alive.
    #[inline]
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        let upgraded = self.upgrade()?;
        let borrowed = upgraded.borrow();
        Some(f(&borrowed))
    }

    /// Returns `true` if the underlying value has been dropped (no strong refs remain).
    #[inline]
    pub fn is_dangling(&self) -> bool {
        self.0.strong_count() == 0
    }
}
impl<T> ArcRecursion<T> {
    /// Try to upgrade the weak reference to [`ArcRecursive<T>`].
    #[inline]
    pub fn upgrade(&self) -> Option<ArcRecursive<T>> {
        self.0.upgrade().map(ArcRecursive)
    }

    /// Access the recursive value in one step, if it is still alive.
    #[inline]
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        let upgraded = self.upgrade()?;
        let guard = upgraded.lock().ok()?;
        let value = guard.as_ref()?;
        Some(f(value))
    }

    /// Returns `true` if the underlying value has been dropped (no strong refs remain).
    #[inline]
    pub fn is_dangling(&self) -> bool {
        self.0.strong_count() == 0
    }
}
impl<T> ArcWeakAnchor<T> {
    /// Try to upgrade the weak reference to [`Arc<T>`].
    /// Returns [`None`] if the value has been dropped.
    #[inline]
    pub fn upgrade(&self) -> Option<Arc<T>> {
        self.0.upgrade()
    }

    /// Returns `true` if the underlying value has been dropped (no strong refs remain).
    #[inline]
    pub fn is_dangling(&self) -> bool {
        self.0.strong_count() == 0
    }
}

// ===== Pointer-equality PartialEq/Eq =====

impl<T> PartialEq for RcAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Eq for RcAnchor<T> {}

impl<T> PartialEq for ArcAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Eq for ArcAnchor<T> {}

impl<T> PartialEq for RcWeakAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0.upgrade(), other.0.upgrade()) {
            (Some(a), Some(b)) => Rc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}
impl<T> Eq for RcWeakAnchor<T> {}

impl<T> PartialEq for RcRecursion<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0.upgrade(), other.0.upgrade()) {
            (Some(a), Some(b)) => Rc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}
impl<T> Eq for RcRecursion<T> {}

impl<T> PartialEq for ArcWeakAnchor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0.upgrade(), other.0.upgrade()) {
            (Some(a), Some(b)) => Arc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}
impl<T> PartialEq for RcRecursive<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Eq for RcRecursive<T> {}

impl<T> PartialEq for ArcRecursion<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0.upgrade(), other.0.upgrade()) {
            (Some(a), Some(b)) => Arc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}
impl<T> Eq for ArcRecursion<T> {}

impl<T> PartialEq for ArcRecursive<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Eq for ArcRecursive<T> {}
impl<T> Eq for ArcWeakAnchor<T> {}

// ===== Debug =====

impl<T> fmt::Debug for RcAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RcAnchor({:p})", Rc::as_ptr(&self.0))
    }
}
impl<T> fmt::Debug for ArcAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArcAnchor({:p})", Arc::as_ptr(&self.0))
    }
}
impl<T> fmt::Debug for RcWeakAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(rc) = self.0.upgrade() {
            write!(f, "RcWeakAnchor(upgrade={:p})", Rc::as_ptr(&rc))
        } else {
            write!(f, "RcWeakAnchor(dangling)")
        }
    }
}
impl<T> fmt::Debug for ArcWeakAnchor<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(arc) = self.0.upgrade() {
            write!(f, "ArcWeakAnchor(upgrade={:p})", Arc::as_ptr(&arc))
        } else {
            write!(f, "ArcWeakAnchor(dangling)")
        }
    }
}
impl<T> fmt::Debug for RcRecursive<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RcRecursive({:p})", Rc::as_ptr(&self.0))
    }
}
impl<T> fmt::Debug for ArcRecursive<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArcRecursive({:p})", Arc::as_ptr(&self.0))
    }
}
impl<T> fmt::Debug for RcRecursion<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(rc) = self.0.upgrade() {
            write!(f, "RcRecursion(upgrade={:p})", Rc::as_ptr(&rc))
        } else {
            write!(f, "RcRecursion(dangling)")
        }
    }
}
impl<T> fmt::Debug for ArcRecursion<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(arc) = self.0.upgrade() {
            write!(f, "ArcRecursion(upgrade={:p})", Arc::as_ptr(&arc))
        } else {
            write!(f, "ArcRecursion(dangling)")
        }
    }
}

// ===== Default =====

impl<T: Default> Default for RcAnchor<T> {
    #[inline]
    fn default() -> Self {
        RcAnchor(Rc::new(T::default()))
    }
}
impl<T: Default> Default for ArcAnchor<T> {
    fn default() -> Self {
        ArcAnchor(Arc::new(T::default()))
    }
}
impl<T: Default> Default for RcRecursive<T> {
    #[inline]
    fn default() -> Self {
        RcRecursive(Rc::new(RefCell::new(Some(T::default()))))
    }
}
impl<T: Default> Default for ArcRecursive<T> {
    fn default() -> Self {
        ArcRecursive(Arc::new(Mutex::new(Some(T::default()))))
    }
}

// -------------------------------
// Deserialize impls
// -------------------------------
impl<'de, T> serde::de::Deserialize<'de> for RcAnchor<T>
where
    T: serde::de::Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct RcAnchorVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for RcAnchorVisitor<T>
        where
            T: serde::de::Deserialize<'de> + 'static,
        {
            type Value = RcAnchor<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an RcAnchor newtype")
            }

            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let anchor_id = anchor_store::current_rc_anchor();
                let existing = match anchor_id {
                    Some(id) => {
                        Some((id, anchor_store::get_rc::<T>(id).map_err(D::Error::custom)?))
                    }
                    None => None,
                };
                if let Some((id, None)) = existing {
                    if anchor_store::rc_anchor_reentrant(id) {
                        return Err(D::Error::custom(
                            "Recursive references require weak anchors",
                        ));
                    }
                }

                let value = T::deserialize(deserializer)?;
                if let Some((_, Some(rc))) = existing {
                    drop(value);
                    return Ok(RcAnchor(rc));
                }
                if let Some((id, None)) = existing {
                    let rc = Rc::new(value);
                    anchor_store::store_rc(id, rc.clone());
                    return Ok(RcAnchor(rc));
                }
                Ok(RcAnchor(Rc::new(value)))
            }
        }

        deserializer.deserialize_newtype_struct("__yaml_rc_anchor", RcAnchorVisitor(PhantomData))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for ArcAnchor<T>
where
    T: serde::de::Deserialize<'de> + Send + Sync + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct ArcAnchorVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for ArcAnchorVisitor<T>
        where
            T: serde::de::Deserialize<'de> + Send + Sync + 'static,
        {
            type Value = ArcAnchor<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an ArcAnchor newtype")
            }

            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let anchor_id = anchor_store::current_arc_anchor();
                let existing = match anchor_id {
                    Some(id) => Some((
                        id,
                        anchor_store::get_arc::<T>(id).map_err(D::Error::custom)?,
                    )),
                    None => None,
                };
                if let Some((id, None)) = existing {
                    if anchor_store::arc_anchor_reentrant(id) {
                        return Err(D::Error::custom(
                            "Recursive references require weak anchors",
                        ));
                    }
                }

                let value = T::deserialize(deserializer)?;
                if let Some((_, Some(arc))) = existing {
                    drop(value);
                    return Ok(ArcAnchor(arc));
                }
                if let Some((id, None)) = existing {
                    let arc = Arc::new(value);
                    anchor_store::store_arc(id, arc.clone());
                    return Ok(ArcAnchor(arc));
                }
                Ok(ArcAnchor(Arc::new(value)))
            }
        }

        deserializer.deserialize_newtype_struct("__yaml_arc_anchor", ArcAnchorVisitor(PhantomData))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for RcRecursive<T>
where
    T: serde::de::Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct RcRecursiveVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for RcRecursiveVisitor<T>
        where
            T: serde::de::Deserialize<'de> + 'static,
        {
            type Value = RcRecursive<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an RcRecursive newtype")
            }

            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let anchor_id = anchor_store::current_rc_recursive_anchor();
                let existing = match anchor_id {
                    Some(id) => Some(
                        (
                            id,
                            anchor_store::get_rc_recursive::<RefCell<Option<T>>>(id)
                                .map_err(D::Error::custom)?,
                        ),
                    ),
                    None => None,
                };
                if let Some((id, None)) = existing {
                    if anchor_store::rc_recursive_reentrant(id) {
                        return Err(D::Error::custom(
                            "Recursive references require weak recursion types",
                        ));
                    }
                }

                if let Some((_, Some(rc))) = existing {
                    let value = T::deserialize(deserializer)?;
                    drop(value);
                    return Ok(RcRecursive(rc));
                }

                if let Some((id, None)) = existing {
                    let rc = Rc::new(RefCell::new(None));
                    anchor_store::store_rc_recursive(id, rc.clone());

                    let value = T::deserialize(deserializer)?;
                    *rc.borrow_mut() = Some(value);
                    return Ok(RcRecursive(rc));
                }

                let value = T::deserialize(deserializer)?;
                Ok(RcRecursive(Rc::new(RefCell::new(Some(value)))))
            }
        }

        deserializer.deserialize_newtype_struct("__yaml_rc_recursive", RcRecursiveVisitor(PhantomData))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for ArcRecursive<T>
where
    T: serde::de::Deserialize<'de> + Send + Sync + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct ArcRecursiveVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for ArcRecursiveVisitor<T>
        where
            T: serde::de::Deserialize<'de> + Send + Sync + 'static,
        {
            type Value = ArcRecursive<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an ArcRecursive newtype")
            }

            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let anchor_id = anchor_store::current_arc_recursive_anchor();
                let existing = match anchor_id {
                    Some(id) => Some(
                        (
                            id,
                            anchor_store::get_arc_recursive::<Mutex<Option<T>>>(id)
                                .map_err(D::Error::custom)?,
                        ),
                    ),
                    None => None,
                };
                if let Some((id, None)) = existing {
                    if anchor_store::arc_recursive_reentrant(id) {
                        return Err(D::Error::custom(
                            "Recursive references require weak recursion types",
                        ));
                    }
                }

                if let Some((_, Some(arc))) = existing {
                    let value = T::deserialize(deserializer)?;
                    drop(value);
                    return Ok(ArcRecursive(arc));
                }

                if let Some((id, None)) = existing {
                    let arc = Arc::new(Mutex::new(None));
                    anchor_store::store_arc_recursive(id, arc.clone());

                    let value = T::deserialize(deserializer)?;
                    *arc.lock().map_err(|_| D::Error::custom("recursive Arc anchor mutex poisoned"))? =
                        Some(value);
                    return Ok(ArcRecursive(arc));
                }

                let value = T::deserialize(deserializer)?;
                Ok(ArcRecursive(Arc::new(Mutex::new(Some(value)))))
            }
        }

        deserializer
            .deserialize_newtype_struct("__yaml_arc_recursive", ArcRecursiveVisitor(PhantomData))
    }
}

// -------------------------------
// Deserialize impls for WEAK anchors (RcWeakAnchor / ArcWeakAnchor)
// -------------------------------
impl<'de, T> serde::de::Deserialize<'de> for RcWeakAnchor<T>
where
    T: serde::de::Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct RcWeakVisitor<T>(PhantomData<T>);
        impl<'de, T> Visitor<'de> for RcWeakVisitor<T>
        where
            T: serde::de::Deserialize<'de> + 'static,
        {
            type Value = RcWeakAnchor<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an RcWeakAnchor referring to a previously defined strong anchor (via alias)",
                )
            }
            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                // Anchor context is established by de.rs when the special name is used.
                let id = anchor_store::current_rc_anchor().ok_or_else(|| {
                    D::Error::custom(
                        "weak Rc anchor must refer to an existing strong anchor via alias",
                    )
                })?;
                // Consume and ignore the inner node to keep the stream in sync (alias replay injects the full target node).
                let _ =
                    <serde::de::IgnoredAny as serde::de::Deserialize>::deserialize(deserializer)?;
                // Look up the strong reference by id and downgrade.
                match anchor_store::get_rc::<T>(id).map_err(D::Error::custom)? {
                    Some(rc) => Ok(RcWeakAnchor(Rc::downgrade(&rc))),
                    None if anchor_store::rc_anchor_reentrant(id) => Err(D::Error::custom(
                        "Recursive references require RcRecursion",
                    )),
                    None => Err(D::Error::custom(
                        "weak Rc anchor refers to unknown anchor id; strong anchor must be defined before weak",
                    )),
                }
            }
        }
        deserializer.deserialize_newtype_struct("__yaml_rc_weak_anchor", RcWeakVisitor(PhantomData))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for ArcWeakAnchor<T>
where
    T: serde::de::Deserialize<'de> + Send + Sync + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct ArcWeakVisitor<T>(PhantomData<T>);
        impl<'de, T> Visitor<'de> for ArcWeakVisitor<T>
        where
            T: serde::de::Deserialize<'de> + Send + Sync + 'static,
        {
            type Value = ArcWeakAnchor<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an ArcWeakAnchor referring to a previously defined strong anchor (via alias)",
                )
            }
            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let id = anchor_store::current_arc_anchor().ok_or_else(|| {
                    D::Error::custom(
                        "weak Arc anchor must refer to an existing strong anchor via alias",
                    )
                })?;
                // Consume and ignore the inner node (alias replay injects the target node events).
                let _ =
                    <serde::de::IgnoredAny as serde::de::Deserialize>::deserialize(deserializer)?;
                match anchor_store::get_arc::<T>(id).map_err(D::Error::custom)? {
                    Some(arc) => Ok(ArcWeakAnchor(Arc::downgrade(&arc))),
                    None if anchor_store::arc_anchor_reentrant(id) => Err(D::Error::custom(
                        "Recursive references require ArcRecursion",
                    )),
                    None => Err(D::Error::custom(
                        "weak Arc anchor refers to unknown anchor id; strong anchor must be defined before weak",
                    )),
                }
            }
        }
        deserializer
            .deserialize_newtype_struct("__yaml_arc_weak_anchor", ArcWeakVisitor(PhantomData))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for RcRecursion<T>
where
    T: serde::de::Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct RcRecursionVisitor<T>(PhantomData<T>);
        impl<'de, T> Visitor<'de> for RcRecursionVisitor<T>
        where
            T: serde::de::Deserialize<'de> + 'static,
        {
            type Value = RcRecursion<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an RcRecursion referring to a previously defined recursive strong anchor (via alias)",
                )
            }
            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let id = anchor_store::current_rc_recursive_anchor().ok_or_else(|| {
                    D::Error::custom(
                        "RcRecursion must refer to an existing recursive strong anchor via alias",
                    )
                })?;
                let _ =
                    <serde::de::IgnoredAny as serde::de::Deserialize>::deserialize(deserializer)?;
                match anchor_store::get_rc_recursive::<RefCell<Option<T>>>(id)
                    .map_err(D::Error::custom)?
                {
                    Some(rc) => Ok(RcRecursion(Rc::downgrade(&rc))),
                    None => Err(D::Error::custom(
                        "RcRecursion refers to unknown recursive anchor id",
                    )),
                }
            }
        }
        deserializer.deserialize_newtype_struct("__yaml_rc_recursion", RcRecursionVisitor(PhantomData))
    }
}

impl<'de, T> serde::de::Deserialize<'de> for ArcRecursion<T>
where
    T: serde::de::Deserialize<'de> + Send + Sync + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct ArcRecursionVisitor<T>(PhantomData<T>);
        impl<'de, T> Visitor<'de> for ArcRecursionVisitor<T>
        where
            T: serde::de::Deserialize<'de> + Send + Sync + 'static,
        {
            type Value = ArcRecursion<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an ArcRecursion referring to a previously defined recursive strong anchor (via alias)",
                )
            }
            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let id = anchor_store::current_arc_recursive_anchor().ok_or_else(|| {
                    D::Error::custom(
                        "ArcRecursion must refer to an existing recursive strong anchor via alias",
                    )
                })?;
                let _ =
                    <serde::de::IgnoredAny as serde::de::Deserialize>::deserialize(deserializer)?;
                match anchor_store::get_arc_recursive::<Mutex<Option<T>>>(id)
                    .map_err(D::Error::custom)?
                {
                    Some(arc) => Ok(ArcRecursion(Arc::downgrade(&arc))),
                    None => Err(D::Error::custom(
                        "ArcRecursion refers to unknown recursive anchor id",
                    )),
                }
            }
        }
        deserializer
            .deserialize_newtype_struct("__yaml_arc_recursion", ArcRecursionVisitor(PhantomData))
    }
}
