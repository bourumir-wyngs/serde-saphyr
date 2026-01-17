use serde::{Deserialize, Serialize};
use serde_saphyr::{ArcRecursion, ArcRecursive, RcRecursion, RcRecursive};

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

fn assert_recursive_outer_arc(outer: &OuterArc) {
    let foo_guard = outer.foo.lock().unwrap();
    let foo = foo_guard.as_ref().expect("foo should be initialized");
    assert_eq!(foo.k1, "One");
    assert_eq!(foo.k2, "Two");
    let k3 = foo.k3.upgrade().expect("k3 should be alive");
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

fn assert_recursive_outer(outer: &Outer) {
    let foo = outer.foo.borrow();
    assert_eq!(foo.k1, "One");
    assert_eq!(foo.k2, "Two");
    let k3 = foo.k3.upgrade().expect("k3 should be alive");
    let k3_name = foo
        .k3
        .with(|next| next.k1.clone())
        .expect("k3 should be alive");
    assert_eq!(k3_name, "One");
    drop(foo);

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

    let _outer_arc = serde_saphyr::from_str::<OuterArc>(yaml)?;
    assert_recursive_outer_arc(&_outer_arc);

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
    assert!(serialized.contains("&a1"), "serialized YAML should define an anchor");
    assert!(serialized.contains("*a1"), "serialized YAML should use an alias");

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
    let crowned = second.crowned_by.upgrade().expect("crowned_by should be alive");
    drop(second);

    let crowned_ref = crowned.borrow();
    assert_eq!(crowned_ref.birth_name, "Aurelian Markus");
    assert_eq!(crowned_ref.regal_name, "Aurelian I");

    Ok(())
}
