use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Enum {
    Unit,
    Newtype(usize),
    Tuple(usize, usize),
    Struct { value: usize },
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Struct {
    w: Enum,
    x: Enum,
    y: Enum,
    z: Enum,
}

#[test]
fn singleton_map_roundtrip() {
    let object = Struct {
        w: Enum::Unit,
        x: Enum::Newtype(1),
        y: Enum::Tuple(1, 1),
        z: Enum::Struct { value: 1 },
    };

    let yaml = serde_saphyr::to_string(&object).unwrap();
    println!("{}", yaml);

    let deserialized: Struct = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(object, deserialized);
}
