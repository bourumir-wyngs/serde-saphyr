use serde_saphyr::{Mapping, Value};

#[test]
fn test_reserve_and_shrink() {
    let mut map = Mapping::new();
    let initial = map.capacity();
    map.reserve(10);
    assert!(map.capacity() >= initial + 10);

    map.insert("a".into(), 1.into());
    map.insert("b".into(), 2.into());
    let cap_before = map.capacity();
    map.shrink_to_fit();
    assert!(map.capacity() <= cap_before);
    assert!(map.capacity() >= map.len());
}

#[test]
fn test_swap_and_shift_remove() {
    let mut map = Mapping::new();
    map.insert("a".into(), 1.into());
    map.insert("b".into(), 2.into());
    map.insert("c".into(), 3.into());

    let removed = map.swap_remove("b");
    assert_eq!(removed, Some(Value::from(2)));
    assert!(!map.contains_key("b"));
    assert_eq!(map.len(), 2);

    let mut map2 = Mapping::new();
    map2.insert("a".into(), 1.into());
    map2.insert("b".into(), 2.into());
    map2.insert("c".into(), 3.into());

    let removed = map2.shift_remove("b");
    assert_eq!(removed, Some(Value::from(2)));
    assert!(!map2.contains_key("b"));
    let keys: Vec<_> = map2.keys().map(|k| k.as_str().unwrap().to_string()).collect();
    assert_eq!(keys, ["a", "c"]);
}

#[test]
fn test_iterators() {
    let mut map = Mapping::new();
    map.insert("x".into(), 1.into());
    map.insert("y".into(), 2.into());

    let keys: Vec<_> = map.keys().map(|k| k.as_str().unwrap().to_string()).collect();
    assert_eq!(keys, ["x", "y"]);
    let values: Vec<_> = map.values().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(values, [1, 2]);
}

#[test]
fn test_default_map_empty() {
    let map = Mapping::default();
    assert!(map.is_empty());
    assert_eq!(map.capacity(), 0);
}
