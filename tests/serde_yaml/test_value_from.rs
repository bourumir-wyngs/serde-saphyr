use std::borrow::Cow;
use serde_saphyr::{Number, Sequence, Value};

#[test]
fn test_value_from_iter_integers() {
    let value = Value::from_iter(1..=3);
    let mut expected = Sequence::new();
    expected.elements = vec![
        Value::Number(Number::from(1), None),
        Value::Number(Number::from(2), None),
        Value::Number(Number::from(3), None),
    ];
    assert_eq!(value, Value::Sequence(expected));
    assert_eq!(serde_saphyr::to_string(&value).unwrap(), "- 1\n- 2\n- 3\n");
}

#[test]
fn test_value_from_iter_strings() {
    let items = vec!["a", "b", "c"];
    let value = Value::from_iter(items.iter().cloned());
    let mut expected = Sequence::new();
    expected.elements = vec![
        Value::String("a".into(), None),
        Value::String("b".into(), None),
        Value::String("c".into(), None),
    ];
    assert_eq!(value, Value::Sequence(expected));
    assert_eq!(serde_saphyr::to_string(&value).unwrap(), "- a\n- b\n- c\n");
}

#[test]
fn test_value_from_slice() {
    let slice: &[i32] = &[1, 2, 3];
    let value = Value::from(slice);
    let mut expected = Sequence::new();
    expected.elements = vec![
        Value::Number(Number::from(1), None),
        Value::Number(Number::from(2), None),
        Value::Number(Number::from(3), None),
    ];
    assert_eq!(value, Value::Sequence(expected));
    assert_eq!(serde_saphyr::to_string(&value).unwrap(), "- 1\n- 2\n- 3\n");
}

#[test]
fn test_value_from_cow_borrowed() {
    let cow: Cow<str> = Cow::Borrowed("hello");
    let value = Value::from(cow);
    assert_eq!(value, Value::String("hello".to_string(), None));
    assert_eq!(serde_saphyr::to_string(&value).unwrap(), "hello\n");
}

#[test]
fn test_value_from_cow_owned() {
    let cow: Cow<str> = Cow::Owned("world".to_string());
    let value = Value::from(cow);
    assert_eq!(value, Value::String("world".to_string(), None));
    assert_eq!(serde_saphyr::to_string(&value).unwrap(), "world\n");
}
