use serde::Deserialize;

// 9FMG: Multi-level Mapping Indent
#[derive(Debug, Deserialize, PartialEq)]
struct Inner {
    c: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Inner2 {
    f: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct A {
    b: Inner,
    e: Inner2,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Root {
    a: A,
    h: String,
}

#[test]
fn yaml_9fmg_multi_level_mapping_indent() {
    let y = "a:\n  b:\n    c: d\n  e:\n    f: g\nh: i\n";
    let r: Root = serde_saphyr::from_str(y).expect("failed to parse 9FMG");
    assert_eq!(r.a.b.c, "d");
    assert_eq!(r.a.e.f, "g");
    assert_eq!(r.h, "i");
}
