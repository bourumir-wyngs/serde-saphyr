#[cfg(feature = "garde")]
use crate::Location;

#[cfg(feature = "garde")]
use std::collections::{HashMap};

#[cfg(feature = "garde")]
pub(crate) type PathKey = garde::error::Path;

#[cfg(feature = "garde")]
pub(crate) struct PathMap {
    pub(crate) map: HashMap<PathKey, Location>,
}

#[cfg(feature = "garde")]
impl PathMap {
    pub(crate) fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub(crate) fn insert(&mut self, path: PathKey, location: Location) {
        self.map.insert(path, location);
    }
}

#[cfg(feature = "garde")]
pub(crate) struct PathRecorder {
    pub(crate) current: PathKey,
    /// Use-site (reference) locations, consistent with `Events::reference_location()`.
    pub(crate) map: PathMap,
    /// Definition-site locations (typically `Ev::location()` from `peek()`).
    pub(crate) defined: PathMap,
}

#[cfg(feature = "garde")]
impl PathRecorder {
    pub(crate) fn new() -> Self {
        Self {
            current: PathKey::empty(),
            map: PathMap::new(),
            defined: PathMap::new(),
        }
    }
}
