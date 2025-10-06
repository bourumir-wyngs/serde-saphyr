use serde_saphyr::{Mapping, Number, Sequence, Value};

#[test]
fn test_value_accessors() {
    let null = Value::Null(None);
    assert!(null.is_null());
    assert!(!null.is_bool());
    assert!(!null.is_number());
    assert!(!null.is_string());
    assert_eq!(null.as_i64(), None);
    assert_eq!(null.as_u64(), None);
    assert_eq!(null.as_f64(), None);
    assert_eq!(null.as_str(), None);

    let boolean = Value::Bool(true, None);
    assert!(!boolean.is_null());
    assert!(boolean.is_bool());
    assert!(!boolean.is_number());
    assert!(!boolean.is_string());
    assert_eq!(boolean.as_bool(), Some(true));
    assert_eq!(boolean.as_i64(), None);
    assert_eq!(boolean.as_u64(), None);
    assert_eq!(boolean.as_f64(), None);
    assert_eq!(boolean.as_str(), None);

    let number_int = Value::Number(Number::from(10), None);
    assert!(!number_int.is_null());
    assert!(!number_int.is_bool());
    assert!(number_int.is_number());
    assert!(!number_int.is_string());
    assert!(number_int.is_i64());
    assert!(number_int.is_u64());
    assert!(!number_int.is_f64());
    assert_eq!(number_int.as_i64(), Some(10));
    assert_eq!(number_int.as_u64(), Some(10));
    assert_eq!(number_int.as_f64(), Some(10.0));
    assert_eq!(number_int.as_str(), None);

    let number_float = Value::Number(Number::from(1.25), None);
    assert!(number_float.is_number());
    assert!(!number_float.is_i64());
    assert!(!number_float.is_u64());
    assert!(number_float.is_f64());
    assert_eq!(number_float.as_f64(), Some(1.25));
    assert_eq!(number_float.as_i64(), None);
    assert_eq!(number_float.as_u64(), None);

    let string = Value::String("hello".to_owned(), None);
    assert!(!string.is_null());
    assert!(!string.is_bool());
    assert!(!string.is_number());
    assert!(string.is_string());
    assert_eq!(string.as_str(), Some("hello"));
    assert_eq!(string.as_i64(), None);
    assert_eq!(string.as_u64(), None);
    assert_eq!(string.as_f64(), None);
}

#[test]
fn test_indexing_returns_null_when_absent() {
    let mut map = Mapping::new();
    map.insert(Value::String("a".into(), None), Value::Number(Number::from(1), None));
    let map_val = Value::Mapping(map);

    assert_eq!(map_val["a"], Value::Number(Number::from(1), None));
    assert_eq!(map_val["missing"], Value::Null(None));

    let seq = Sequence {
        anchor: None,
        elements: vec![Value::Bool(false, None)],
    };
    let seq_val = Value::Sequence(seq);

    assert_eq!(seq_val[0], Value::Bool(false, None));
    assert_eq!(seq_val[1], Value::Null(None));
}

#[test]
fn test_sequence_default() {
    let seq = Sequence::default();
    assert_eq!(None, seq.anchor);
    assert!(seq.elements.is_empty());
    assert_eq!(seq, Sequence::new());
}

#[test]
fn test_sequence_with_anchor() {
    let seq = Sequence::with_anchor("anchor");
    assert_eq!(seq.anchor.as_deref(), Some("anchor"));
    assert!(seq.elements.is_empty());
}

#[test]
fn test_mapping_with_anchor() {
    let map = Mapping::with_anchor("anchor");
    assert_eq!(map.anchor.as_deref(), Some("anchor"));
    assert!(map.is_empty());
}
