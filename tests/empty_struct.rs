use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Empty {}

#[derive(Debug, Deserialize, PartialEq)]
struct Holder {
    name: String,
    empty: Empty,
}

#[test]
fn field_holding_empty_struct_deserializes() {
    let y = "name: John\nempty: {}\n";
    let h: Holder =
        serde_saphyr::from_str(y).expect("failed to deserialize Holder with empty struct field");
    assert_eq!(h.name, "John");
    assert_eq!(h.empty, Empty {});
}
