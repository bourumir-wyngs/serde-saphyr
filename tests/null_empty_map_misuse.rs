#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use std::fmt;

#[derive(Debug)]
struct ValueBeforeKey;

impl<'de> Deserialize<'de> for ValueBeforeKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ValueBeforeKeyVisitor)
    }
}

struct ValueBeforeKeyVisitor;

impl<'de> Visitor<'de> for ValueBeforeKeyVisitor {
    type Value = ValueBeforeKey;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a deliberately misordered map visitor")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let _ = map.next_value::<IgnoredAny>()?;
        Ok(ValueBeforeKey)
    }
}

#[test]
fn null_is_rejected_before_invoking_map_visitor() {
    let err = serde_saphyr::from_str::<ValueBeforeKey>("~").unwrap_err();

    match inner_error(&err) {
        serde_saphyr::Error::Unexpected { expected, location } => {
            assert_eq!(*expected, "mapping start");
            assert_eq!(location.line(), 1);
            assert_eq!(location.column(), 1);
        }
        other => panic!("expected a mapping type error, got {other:?}"),
    }
}

fn inner_error(err: &serde_saphyr::Error) -> &serde_saphyr::Error {
    match err {
        serde_saphyr::Error::WithSnippet { error, .. } => inner_error(error),
        err => err,
    }
}
