use serde::Deserialize;
use serde_saphyr::sf_serde;

#[derive(Debug, Deserialize, PartialEq)]
struct UWrap {
    u: (),
}

#[test]
fn unit_accepts_null_forms() {
    // explicit null
    let y1 = "u: null\n";
    let w1: UWrap = sf_serde::from_str(y1).unwrap();
    assert_eq!(w1, UWrap { u: () });

    // tilde
    let y2 = "u: ~\n";
    let w2: UWrap = sf_serde::from_str(y2).unwrap();
    assert_eq!(w2, UWrap { u: () });

    // empty scalar (key with no value)
    let y3 = "u:\n";
    let w3: UWrap = sf_serde::from_str(y3).unwrap();
    assert_eq!(w3, UWrap { u: () });
}

#[derive(Debug, Deserialize, PartialEq)]
struct US; // unit struct

#[derive(Debug, Deserialize, PartialEq)]
struct WrapUS {
    s: US,
}

#[test]
fn unit_struct_accepts_null_forms_and_empty_map() {
    // explicit null
    let y1 = "s: null\n";
    let w1: WrapUS = sf_serde::from_str(y1).unwrap();
    assert_eq!(w1, WrapUS { s: US });

    // tilde
    let y2 = "s: ~\n";
    let w2: WrapUS = sf_serde::from_str(y2).unwrap();
    assert_eq!(w2, WrapUS { s: US });

    // empty scalar (key with no value)
    let y3 = "s:\n";
    let w3: WrapUS = sf_serde::from_str(y3).unwrap();
    assert_eq!(w3, WrapUS { s: US });

    // empty mapping
    let y4 = "s: {}\n";
    let w4: WrapUS = sf_serde::from_str(y4).unwrap();
    assert_eq!(w4, WrapUS { s: US });
}
