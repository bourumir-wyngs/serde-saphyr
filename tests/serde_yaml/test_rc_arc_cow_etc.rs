use serde::{Deserialize, Serialize};
use serde_saphyr::{from_str, to_string};
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct BuiltInTypes {
    // Rc and Arc require "rc" feature that is in dev dependencies of this library.
    rc: Rc<String>,
    arc: Arc<Vec<u8>>,

    boxed: Box<i32>,
    opt_some: Option<String>,
    opt_none: Option<String>,
    res_ok: Result<u8, String>,
    res_err: Result<u8, String>,
    cow_owned: Cow<'static, str>,
    cow_borrowed: Cow<'static, str>,
}

#[test]
fn test_serde_saphyr_builtin_types() {
    let original = BuiltInTypes {
        rc: Rc::new("Rc data".to_string()),
        arc: Arc::new(vec![1, 2, 3]),
        boxed: Box::new(42),
        opt_some: Some("hello".to_string()),
        opt_none: None,
        res_ok: Ok(255),
        res_err: Err("an error".to_string()),
        cow_owned: Cow::Owned("owned cow".to_string()),
        cow_borrowed: Cow::Borrowed("borrowed cow"),
    };

    // Serialize
    let serialized = to_string(&original).expect("Serialization failed");

    // Deserialize
    let deserialized: BuiltInTypes =
        from_str(&serialized).expect(&format!("Deserialization failed: {}", serialized));

    // Verify entire struct equality
    assert_eq!(original, deserialized, "YAML: {serialized}");

    // Verify specific pointer content equality explicitly (just in case)
    assert_eq!(*original.rc, *deserialized.rc);
    assert_eq!(*original.arc, *deserialized.arc);
}
