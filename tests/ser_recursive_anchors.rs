#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_saphyr::{
    ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor, RcAnchor, RcRecursion, RcRecursive,
    RcWeakAnchor, SerializeError, to_string, to_string_with_options,
};

type AnchorGenerator = fn(usize) -> String;

fn serialize_with_anchor_generator(generator: AnchorGenerator) -> Result<String, SerializeError> {
    let shared = Rc::new(42i32);
    let values = vec![RcAnchor(shared.clone()), RcAnchor(shared)];
    let options = serde_saphyr::ser_options! {
        anchor_generator: Some(generator),
    };
    to_string_with_options(&values, options)
}

#[test]
fn arc_weak_anchor_dangling_emits_null() {
    let weak = {
        let arc = Arc::new(42i32);
        ArcWeakAnchor(Arc::downgrade(&arc))
        // arc dropped here, weak becomes dangling
    };
    let yaml = to_string(&weak).unwrap();
    assert!(
        yaml.contains("null"),
        "expected null for dangling weak: {yaml}"
    );
}

#[test]
fn arc_recursion_dangling_emits_null() {
    let weak = {
        let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(None)));
        ArcRecursion::from(&arc_rec)
        // arc_rec dropped here
    };
    let yaml = to_string(&weak).unwrap();
    assert!(
        yaml.contains("null"),
        "expected null for dangling arc recursion: {yaml}"
    );
}

#[test]
fn arc_recursive_serializes_value() {
    let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(Some(99))));
    let yaml = to_string(&arc_rec).unwrap();
    assert!(yaml.contains("99"), "expected 99 in: {yaml}");
}

#[test]
fn many_anchors_produce_multi_digit_ids() {
    // Create enough anchors to get id >= 10
    let values: Vec<RcAnchor<i32>> = (0..15).map(|i| RcAnchor(std::rc::Rc::new(i))).collect();
    let yaml = to_string(&values).unwrap();
    // Should contain anchor names like &a10 or higher
    assert!(
        yaml.contains("&a10") || yaml.contains("&a11"),
        "yaml: {yaml}"
    );
}

#[test]
fn rc_weak_anchor_dangling_emits_null() {
    use std::rc::Rc;
    let weak = {
        let rc = Rc::new(42i32);
        RcWeakAnchor(Rc::downgrade(&rc))
        // rc dropped here
    };
    let yaml = to_string(&weak).unwrap();
    assert!(
        yaml.contains("null"),
        "expected null for dangling rc weak: {yaml}"
    );
}

#[test]
fn rc_recursion_dangling_emits_null() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let weak = {
        let rc_rec = RcRecursive::<i32>(Rc::new(RefCell::new(None)));
        RcRecursion::from(&rc_rec)
        // rc_rec dropped here
    };
    let yaml = to_string(&weak).unwrap();
    assert!(
        yaml.contains("null"),
        "expected null for dangling rc recursion: {yaml}"
    );
}

#[test]
fn alias_as_map_value_not_at_line_start() {
    use std::rc::Rc;
    #[derive(Serialize)]
    struct S {
        a: RcAnchor<i32>,
        b: RcAnchor<i32>,
    }
    let shared = Rc::new(42i32);
    let yaml = to_string(&S {
        a: RcAnchor(shared.clone()),
        b: RcAnchor(shared.clone()),
    })
    .unwrap();
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

#[test]
fn arc_anchor_alias_reuse() {
    let shared = std::sync::Arc::new(42i32);
    let a = ArcAnchor(shared.clone());
    let b = ArcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string(&v).unwrap();
    // Second occurrence should be an alias (*a0 or similar)
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

#[test]
fn rc_anchor_alias_reuse() {
    use std::rc::Rc;
    let shared = Rc::new("hello");
    let a = RcAnchor(shared.clone());
    let b = RcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

#[test]
fn rc_weak_anchor_live_emits_value() {
    use std::rc::Rc;
    let rc = Rc::new(99i32);
    let weak = RcWeakAnchor(Rc::downgrade(&rc));
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("99"), "expected value: {yaml}");
    drop(rc);
}

#[test]
fn arc_weak_anchor_live_emits_value() {
    let arc = Arc::new(77i32);
    let weak = ArcWeakAnchor(Arc::downgrade(&arc));
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("77"), "expected value: {yaml}");
    drop(arc);
}

#[test]
fn rc_recursion_live_emits_value() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let rc_rec = RcRecursive::<i32>(Rc::new(RefCell::new(Some(55))));
    let weak = RcRecursion::from(&rc_rec);
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("55"), "expected value: {yaml}");
}

#[test]
fn arc_recursion_live_emits_value() {
    let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(Some(33))));
    let weak = ArcRecursion::from(&arc_rec);
    let yaml = to_string(&weak).unwrap();
    assert!(yaml.contains("33"), "expected value: {yaml}");
}

#[test]
fn custom_anchor_generator_rejects_unsupported_names() {
    let invalid_generators: [(&str, AnchorGenerator); 11] = [
        ("empty", |_| String::new()),
        ("257 ASCII bytes", |_| "a".repeat(257)),
        ("258 UTF-8 bytes", |_| "é".repeat(129)),
        ("ASCII whitespace", |_| "bad name".to_string()),
        ("Unicode whitespace", |_| "bad\u{a0}name".to_string()),
        ("control character", |_| "bad\u{7}name".to_string()),
        ("opening bracket", |_| "bad[name".to_string()),
        ("closing bracket", |_| "bad]name".to_string()),
        ("opening brace", |_| "bad{name".to_string()),
        ("closing brace", |_| "bad}name".to_string()),
        ("comma", |_| "bad,name".to_string()),
    ];

    for (case, generator) in invalid_generators {
        let error = serialize_with_anchor_generator(generator).unwrap_err();
        match error {
            SerializeError::InvalidOptions(message) => {
                assert!(
                    message.contains("custom anchor generator"),
                    "{case}: {message}"
                );
                assert!(message.contains("anchor id 1"), "{case}: {message}");
            }
            other => panic!("{case}: expected InvalidOptions, got {other:?}"),
        }
    }
}

#[test]
fn custom_anchor_generator_accepts_supported_names_at_byte_limit() {
    let valid_generators: [(&str, AnchorGenerator); 3] = [
        ("256 ASCII bytes", |_| "a".repeat(256)),
        ("256 UTF-8 bytes", |_| "é".repeat(128)),
        ("Unicode and punctuation", |_| {
            "release-1.2:β/日本語?".to_string()
        }),
    ];

    for (case, generator) in valid_generators {
        let yaml = serialize_with_anchor_generator(generator).unwrap();
        let name = generator(1);
        assert!(yaml.contains(&format!("&{name}")), "{case}: {yaml}");
        assert!(yaml.contains(&format!("*{name}")), "{case}: {yaml}");

        let parsed: Vec<RcAnchor<i32>> = serde_saphyr::from_str(&yaml).unwrap();
        assert!(Rc::ptr_eq(&parsed[0].0, &parsed[1].0), "{case}: {yaml}");
    }
}

#[test]
fn custom_anchor_generator_is_not_called_without_anchors() {
    let options = serde_saphyr::ser_options! {
        anchor_generator: Some(|_| String::new()),
    };

    assert_eq!(to_string_with_options(&42, options).unwrap(), "42\n");
}

#[test]
fn custom_anchor_generator_rejects_unsupported_name_for_live_weak_anchor() {
    let shared = Rc::new(42i32);
    let weak = RcWeakAnchor(Rc::downgrade(&shared));
    let options = serde_saphyr::ser_options! {
        anchor_generator: Some(|_| String::new()),
    };

    assert!(matches!(
        to_string_with_options(&weak, options),
        Err(SerializeError::InvalidOptions(_))
    ));
}

#[test]
fn arc_recursive_uninitialized_returns_error() {
    let arc_rec = ArcRecursive::<i32>(Arc::new(Mutex::new(None)));
    // Serializing ArcRecursive with None should return an error
    let result = to_string(&arc_rec);
    assert!(
        result.is_err(),
        "expected error for uninitialized ArcRecursive"
    );
}

#[test]
fn rc_recursive_uninitialized_returns_error() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let rc_rec = RcRecursive::<i32>(Rc::new(RefCell::new(None)));
    let result = to_string(&rc_rec);
    assert!(
        result.is_err(),
        "expected error for uninitialized RcRecursive"
    );
}

#[test]
fn custom_anchor_generator_used() {
    use std::rc::Rc;
    let opts = serde_saphyr::ser_options! {
        anchor_generator: Some(|id| format!("myanchor{id}")),
    };
    let shared = Rc::new(42i32);
    let a = RcAnchor(shared.clone());
    let b = RcAnchor(shared.clone());
    let v = vec![a, b];
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(
        yaml.contains("myanchor"),
        "expected custom anchor name: {yaml}"
    );
    assert!(yaml.contains('*'), "expected alias: {yaml}");
}

#[test]
fn rc_recursive_serializes_with_anchor() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Rc::new(RefCell::new(Some(Node { val: 10 })));
    let anchor = RcRecursive(inner.clone());
    let yaml = to_string(&anchor).unwrap();
    assert!(yaml.contains("val: 10"), "expected value: {yaml}");
    assert!(yaml.contains("&a1"), "expected anchor: {yaml}");
}

#[test]
fn arc_recursive_serializes_with_anchor() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Arc::new(Mutex::new(Some(Node { val: 20 })));
    let anchor = ArcRecursive(inner.clone());
    let yaml = to_string(&anchor).unwrap();
    assert!(yaml.contains("val: 20"), "expected value: {yaml}");
    assert!(yaml.contains("&a1"), "expected anchor: {yaml}");
}

#[test]
fn rc_recursion_present_serializes() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Rc::new(RefCell::new(Some(Node { val: 30 })));
    let weak = Rc::downgrade(&inner);
    let recur = RcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("val: 30"), "expected value: {yaml}");
}

#[test]
fn rc_recursion_dangling_serializes_as_null() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let weak = {
        let inner = Rc::new(RefCell::new(Some(Node { val: 0 })));
        Rc::downgrade(&inner)
    };
    let recur = RcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn arc_recursion_present_serializes() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let inner = Arc::new(Mutex::new(Some(Node { val: 40 })));
    let weak = Arc::downgrade(&inner);
    let recur = ArcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("val: 40"), "expected value: {yaml}");
}

#[test]
fn arc_recursion_dangling_serializes_as_null() {
    #[derive(Serialize)]
    struct Node {
        val: i32,
    }
    let weak = {
        let inner = Arc::new(Mutex::new(Some(Node { val: 0 })));
        Arc::downgrade(&inner)
    };
    let recur = ArcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn arc_weak_anchor_dangling_serializes_as_null() {
    #[derive(Serialize, Clone)]
    struct N {
        v: i32,
    }
    let weak = {
        let s = Arc::new(N { v: 1 });
        Arc::downgrade(&s)
    };
    let yaml = to_string(&ArcWeakAnchor(weak)).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn custom_anchor_generator() {
    let shared = Rc::new(42i32);
    let v = vec![RcAnchor(shared.clone()), RcAnchor(shared)];
    let opts = serde_saphyr::ser_options! {
        anchor_generator: Some(|id| format!("custom{}", id)),
    };
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(yaml.contains("&custom1"), "expected custom anchor: {yaml}");
    assert!(yaml.contains("*custom1"), "expected custom alias: {yaml}");
}

#[test]
fn rc_recursive_not_initialized_errors() {
    let inner: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
    let anchor = RcRecursive(inner);
    let err = to_string(&anchor).unwrap_err();
    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { msg }
            if msg == "recursive Rc anchor not initialized"
    ));
}

#[test]
fn arc_recursive_not_initialized_errors() {
    let inner: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let anchor = ArcRecursive(inner);
    let err = to_string(&anchor).unwrap_err();
    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { msg }
            if msg == "recursive Arc anchor not initialized"
    ));
}

#[test]
fn rc_recursion_with_none_inner_serializes_as_null_value() {
    // RcRecursion -> present=true -> RcRecursivePayload -> inner is None -> serialize_unit
    let inner: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&inner);
    let recur = RcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("null"), "expected null: {yaml}");
}

#[test]
fn arc_recursion_with_none_inner_serializes_as_null_value() {
    let inner: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let weak = Arc::downgrade(&inner);
    let recur = ArcRecursion(weak);
    let yaml = to_string(&recur).unwrap();
    assert!(yaml.contains("null"), "expected null: {yaml}");
}
