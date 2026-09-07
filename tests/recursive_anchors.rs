#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::{Deserialize, Serialize};
use serde_saphyr::{
    ArcAnchor, ArcRecursion, ArcRecursive, DuplicateKeyPolicy, RcAnchor, RcRecursion, RcRecursive,
};
use std::{collections::BTreeMap, rc::Rc, sync::Arc};

#[derive(Deserialize)]
struct Node {
    next: RcRecursion<Node>,
    #[serde(default)]
    value: u32,
}

#[derive(Deserialize)]
struct NodeArc {
    next: ArcRecursion<NodeArc>,
    #[serde(default)]
    value: u32,
}

#[test]
fn last_wins_integer_key_preserves_rc_recursive_anchor() {
    let yaml = "1: &node\n  next: *node\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let maps: [BTreeMap<u32, RcRecursive<Node>>; 2] = [
        serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap(),
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options).unwrap(),
    ];

    for nodes in maps {
        assert_eq!(nodes.len(), 1);
        let node = &nodes[&1];
        let next = node.borrow().next.upgrade().expect("next should be alive");
        assert!(Rc::ptr_eq(&node.0, &next.0));
    }
}

#[test]
fn last_wins_integer_key_preserves_arc_recursive_anchor() {
    let yaml = "1: &node\n  next: *node\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let maps: [BTreeMap<u32, ArcRecursive<NodeArc>>; 2] = [
        serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap(),
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options).unwrap(),
    ];

    for nodes in maps {
        assert_eq!(nodes.len(), 1);
        let node = &nodes[&1];
        let next = node
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .next
            .upgrade()
            .expect("next should be alive");
        assert!(Arc::ptr_eq(&node.0, &next.0));
    }
}

#[test]
fn last_wins_duplicate_integer_keys_preserve_rc_recursive_anchor_identity() {
    let yaml = "0x1: &first\n  next: *first\n  value: 1\n\
                1: &last\n  next: *last\n  value: 2\n\
                2: *last\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let nodes: BTreeMap<u32, RcRecursive<Node>> =
        serde_saphyr::from_str_with_options(yaml, options).unwrap();

    assert_eq!(nodes.len(), 2);
    let node = &nodes[&1];
    assert_eq!(node.borrow().value, 2);
    let next = node.borrow().next.upgrade().expect("next should be alive");
    assert!(Rc::ptr_eq(&node.0, &next.0));
    assert!(Rc::ptr_eq(&node.0, &nodes[&2].0));
}

#[test]
fn last_wins_duplicate_integer_keys_preserve_arc_recursive_anchor_identity() {
    let yaml = "0x1: &first\n  next: *first\n  value: 1\n\
                1: &last\n  next: *last\n  value: 2\n\
                2: *last\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let nodes: BTreeMap<u32, ArcRecursive<NodeArc>> =
        serde_saphyr::from_str_with_options(yaml, options).unwrap();

    assert_eq!(nodes.len(), 2);
    let node = &nodes[&1];
    let next = {
        let guard = node.lock().unwrap();
        let value = guard.as_ref().unwrap();
        assert_eq!(value.value, 2);
        value.next.upgrade().expect("next should be alive")
    };
    assert!(Arc::ptr_eq(&node.0, &next.0));
    assert!(Arc::ptr_eq(&node.0, &nodes[&2].0));
}

#[test]
fn last_wins_integer_key_rejects_recursion_without_wrappers() {
    let yaml = "1: &node\n  next: *node\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let error =
        serde_saphyr::from_str_with_options::<BTreeMap<u32, serde_json::Value>>(yaml, options)
            .unwrap_err();

    assert!(
        matches!(
            error.without_snippet(),
            serde_saphyr::Error::RecursiveReferencesRequireWeakTypes { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn last_wins_integer_key_rejects_recursive_strong_anchors() {
    #[derive(Deserialize)]
    struct RcNode {
        #[allow(dead_code)]
        next: RcAnchor<RcNode>,
    }

    #[derive(Deserialize)]
    struct ArcNode {
        #[allow(dead_code)]
        next: ArcAnchor<ArcNode>,
    }

    let yaml = "1: &node\n  next: *node\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let errors = [
        serde_saphyr::from_str_with_options::<BTreeMap<u32, RcAnchor<RcNode>>>(
            yaml,
            options.clone(),
        )
        .expect_err("recursive RcAnchor should be rejected"),
        serde_saphyr::from_str_with_options::<BTreeMap<u32, ArcAnchor<ArcNode>>>(yaml, options)
            .expect_err("recursive ArcAnchor should be rejected"),
    ];

    for error in errors {
        assert!(
            matches!(
                error.without_snippet(),
                serde_saphyr::Error::RecursiveReferencesRequireWeakTypes { .. }
            ),
            "{error:?}"
        );
    }
}

#[test]
fn recursive_anchor_alias_in_unknown_field_is_ignored() {
    #[derive(Deserialize)]
    struct Doc {
        foo: RcRecursive<Node>,
    }

    #[derive(Deserialize)]
    struct DocArc {
        foo: ArcRecursive<NodeArc>,
    }

    let yaml = "foo: &node { next: *node }\nextra: *node\n";
    for duplicate_keys in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        let options = serde_saphyr::options! { duplicate_keys: duplicate_keys };
        let doc: Doc = serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
        let next = doc
            .foo
            .borrow()
            .next
            .upgrade()
            .expect("next should be alive");
        assert!(Rc::ptr_eq(&doc.foo.0, &next.0));

        let doc: DocArc = serde_saphyr::from_str_with_options(yaml, options).unwrap();
        let next = doc
            .foo
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .next
            .upgrade()
            .expect("next should be alive");
        assert!(Arc::ptr_eq(&doc.foo.0, &next.0));
    }
}

#[test]
fn last_wins_integer_key_preserves_recursive_merge_anchor() {
    let yaml = "&node\n1: 10\n<<: *node\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let expected = BTreeMap::from([(1, 10)]);

    let node: RcRecursive<BTreeMap<u32, u32>> =
        serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
    assert_eq!(*node.borrow(), expected);

    let node: ArcRecursive<BTreeMap<u32, u32>> =
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
    assert_eq!(node.lock().unwrap().as_ref().unwrap(), &expected);
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Foo {
    k1: String,
    k2: String,
    // Recursive references require weak anchors
    k3: RcRecursion<Foo>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Outer {
    foo: RcRecursive<Foo>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct FooArc {
    k1: String,
    k2: String,
    // Recursive references require weak anchors
    k3: ArcRecursion<FooArc>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct OuterArc {
    foo: ArcRecursive<FooArc>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct King {
    from: usize,
    birth_name: String,
    regal_name: String,
    crowned_by: RcRecursion<King>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Kingdom {
    kings: Vec<RcRecursive<King>>,
}

#[track_caller]
fn assert_recursive_outer_arc(outer: &OuterArc) {
    let foo_guard = outer.foo.lock().unwrap();
    let foo_ref = foo_guard.as_ref().expect("foo_ref should be initialized");
    assert_eq!(foo_ref.k1, "One");
    assert_eq!(foo_ref.k2, "Two");
    let k3 = foo_ref.k3.upgrade().expect("k3 should be alive");
    let k3_weak = ArcRecursion::from(&k3);
    drop(foo_guard);

    let k3_name = k3_weak
        .with(|next| next.k1.clone())
        .expect("k3 should be alive");
    assert_eq!(k3_name, "One");

    let k3_guard = k3.lock().unwrap();
    let k3_ref = k3_guard.as_ref().expect("k3 should be initialized");
    assert_eq!(k3_ref.k1, "One");
    assert_eq!(k3_ref.k2, "Two");
    let k3k3 = k3_ref.k3.upgrade().expect("k3.k3 should be alive");
    drop(k3_guard);

    let k3k3_guard = k3k3.lock().unwrap();
    let k3k3_ref = k3k3_guard.as_ref().expect("k3.k3 should be initialized");
    assert_eq!(k3k3_ref.k1, "One");
    assert_eq!(k3k3_ref.k2, "Two");
    let k3k3_weak = ArcRecursion::from(&k3k3);
    drop(k3k3_guard);

    let k3k3_name = k3k3_weak
        .with(|next| next.k1.clone())
        .expect("k3.k3 should be alive");
    assert_eq!(k3k3_name, "One");
    // We have infinite recursion here, be careful with this.
}

#[track_caller]
fn assert_recursive_outer(outer: &Outer) {
    let foo_ref = outer.foo.borrow();
    assert_eq!(foo_ref.k1, "One");
    assert_eq!(foo_ref.k2, "Two");
    let k3 = foo_ref.k3.upgrade().expect("k3 should be alive");
    let k3_name = foo_ref
        .k3
        .with(|next| next.k1.clone())
        .expect("k3 should be alive");
    assert_eq!(k3_name, "One");
    drop(foo_ref);

    let k3_ref = k3.borrow();
    assert_eq!(k3_ref.k1, "One");
    assert_eq!(k3_ref.k2, "Two");
    let k3k3 = k3_ref.k3.upgrade().expect("k3.k3 should be alive");
    drop(k3_ref);

    let k3k3_ref = k3k3.borrow();
    assert_eq!(k3k3_ref.k1, "One");
    assert_eq!(k3k3_ref.k2, "Two");
    let k3k3_name = k3k3_ref
        .k3
        .with(|next| next.k1.clone())
        .expect("k3.k3 should be alive");
    assert_eq!(k3k3_name, "One");
    // We have infinite recursion here, be careful with this.
}

#[test]
pub fn test_recursive_anchors() -> anyhow::Result<()> {
    let yaml = r#"
foo: &anchor
 k1: "One"
 k2: "Two"
 k3: *anchor
"#;

    let outer = serde_saphyr::from_str::<Outer>(yaml)?;
    assert_recursive_outer(&outer);

    let outer_arc = serde_saphyr::from_str::<OuterArc>(yaml)?;
    assert_recursive_outer_arc(&outer_arc);

    Ok(())
}

#[test]
pub fn test_recursive_anchors_serialize_roundtrip() -> anyhow::Result<()> {
    let yaml = r#"
foo: &anchor
 k1: "One"
 k2: "Two"
 k3: *anchor
"#;

    let outer = serde_saphyr::from_str::<Outer>(yaml)?;
    let serialized = serde_saphyr::to_string(&outer)?;
    assert!(
        serialized.contains("&a1"),
        "serialized YAML should define an anchor"
    );
    assert!(
        serialized.contains("*a1"),
        "serialized YAML should use an alias"
    );

    let roundtrip = serde_saphyr::from_str::<Outer>(&serialized)?;
    assert_recursive_outer(&roundtrip);

    let outer_arc = serde_saphyr::from_str::<OuterArc>(yaml)?;
    let serialized_arc = serde_saphyr::to_string(&outer_arc)?;
    assert!(
        serialized_arc.contains("&a1"),
        "serialized YAML should define an anchor"
    );
    assert!(
        serialized_arc.contains("*a1"),
        "serialized YAML should use an alias"
    );

    let roundtrip_arc = serde_saphyr::from_str::<OuterArc>(&serialized_arc)?;
    assert_recursive_outer_arc(&roundtrip_arc);

    Ok(())
}

#[test]
pub fn test_recursive_anchor_alias_across_nodes() -> anyhow::Result<()> {
    let yaml = r#"
kings:
  - &markus
    from: 1920
    birth_name: "Aurelian Markus"
    regal_name: "Aurelian I"
    crowned_by: *markus

  - &orlan
    from: 1950
    birth_name: "Benedict Orlan"
    regal_name: "Benedict I"
    crowned_by: *markus
"#;

    let kingdom = serde_saphyr::from_str::<Kingdom>(yaml)?;
    assert_eq!(kingdom.kings.len(), 2);

    let first = kingdom.kings[0].borrow();
    assert_eq!(first.birth_name, "Aurelian Markus");
    assert_eq!(first.regal_name, "Aurelian I");
    drop(first);

    let second = kingdom.kings[1].borrow();
    assert_eq!(second.birth_name, "Benedict Orlan");
    assert_eq!(second.regal_name, "Benedict I");
    let crowned = second
        .crowned_by
        .upgrade()
        .expect("crowned_by should be alive");
    drop(second);

    let crowned_ref = crowned.borrow();
    assert_eq!(crowned_ref.birth_name, "Aurelian Markus");
    assert_eq!(crowned_ref.regal_name, "Aurelian I");

    Ok(())
}
