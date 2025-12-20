use serde_json::Value;

#[test]
fn serde_json_value_preserves_mapping_key_order_from_yaml() {
    // serde_json is built with `preserve_order`, so `Value::Object` uses an order-preserving map.
    // This test asserts that our YAML deserializer feeds map entries to Serde in source order.
    let yaml = r#"a: 1
b: 2
c: 3
"#;

    let v: Value = serde_saphyr::from_str(yaml).unwrap();
    let obj = v.as_object().expect("expected a top-level mapping");

    let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
    assert_eq!(keys, vec!["a", "b", "c"]);
}

#[test]
fn serde_json_value_preserves_order_for_flow_mapping_and_nested_objects() {
    // Also cover flow (JSON-like) mapping syntax and a nested mapping.
    let yaml = r#"root: { z: 0, a: 1, m: 2 }
inner:
  k3: { b: 2, a: 1 }
  k2: 0
"#;

    let v: Value = serde_saphyr::from_str(yaml).unwrap();
    let top = v.as_object().expect("expected a top-level mapping");

    let top_keys: Vec<&str> = top.keys().map(|k| k.as_str()).collect();
    assert_eq!(top_keys, vec!["root", "inner"]);

    let root = top
        .get("root")
        .and_then(Value::as_object)
        .expect("expected root to be a mapping");
    let root_keys: Vec<&str> = root.keys().map(|k| k.as_str()).collect();
    assert_eq!(root_keys, vec!["z", "a", "m"]);

    let inner = top
        .get("inner")
        .and_then(Value::as_object)
        .expect("expected inner to be a mapping");
    let inner_keys: Vec<&str> = inner.keys().map(|k| k.as_str()).collect();
    assert_eq!(inner_keys, vec!["k3", "k2"]);

    let k3 = inner
        .get("k3")
        .and_then(Value::as_object)
        .expect("expected inner.k3 to be a mapping");
    let k3_keys: Vec<&str> = k3.keys().map(|k| k.as_str()).collect();
    assert_eq!(k3_keys, vec!["b", "a"]);
}
