use serde_saphyr::{Sequence, Value};
use serde_saphyr::Mapping;

#[test]
fn test_alias_serialization_errors_without_anchor() {
    let value = Value::Alias("anchor".to_string());
    let err = serde_saphyr::to_string(&value).unwrap_err();
    assert_eq!(
        err.to_string(),
        "reference to non existing anchor [anchor]"
    );
}

#[test]
fn test_alias_in_sequence_resolves() {
    use serde_saphyr::value::Sequence;

    let value = Value::Sequence(Sequence {
        anchor: None,
        elements: vec![
            Value::Number(1.into(), Some("id".to_string())),
            Value::Alias("id".to_string()),
        ],
    });

    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml, "- &id 1\n- *id\n");
}

#[test]
fn test_alias_in_mapping_branch() {

    let mut mapping = Mapping::new();
    mapping.insert(
        Value::String("a".to_string(), None),
        Value::String("foo".to_string(), Some("id".to_string())),
    );
    mapping.insert(
        Value::String("b".to_string(), None),
        Value::Alias("id".to_string()),
    );

    let value = Value::Mapping(mapping);
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml, "a: &id foo\nb: *id\n");
}

// We write two values as list. One is defined as value "referenced" and also has the anchor
// "ref_value_anchor". Another is a map in entries "a" = "b" an "c" = &ref_value_anchor.
// We first write this structure, then print it, and then parse and check that the second
// structure has its anchor properly resolved.
#[test]
fn test_alias_in_sequence_resolves_2() {
    let referenced = Value::String(
        "referenced".to_string(),
        Some("ref_value_anchor".to_string()),
    );

    let mut map = Mapping::new();
    map.insert(
        Value::String("a".to_string(), None),
        Value::String("b".to_string(), None),
    );
    map.insert(
        Value::String("c".to_string(), None),
        Value::Alias("ref_value_anchor".to_string()),
    );

    let seq = Value::Sequence(Sequence {
        anchor: None,
        elements: vec![referenced.clone(), Value::Mapping(map)],
    });

    let yaml = serde_saphyr::to_string(&seq).unwrap();
    assert_eq!(
        yaml,
        "- &ref_value_anchor referenced\n- a: b\n  c: *ref_value_anchor\n",
    );
    println!("{}", yaml);

    let parsed: Value = serde_saphyr::from_str_value(&yaml).unwrap();
    let parsed_seq = parsed.as_sequence().unwrap();
    assert_eq!(
        parsed_seq.elements[0],
        Value::String(
            "referenced".to_string(),
            Some("ref_value_anchor".to_string())
        ),
    );
    let parsed_map = parsed_seq.elements[1].as_mapping().unwrap();
    assert_eq!(
        parsed_map.get("c"),
        Some(&Value::String("referenced".to_string(), None)),
    );
}
