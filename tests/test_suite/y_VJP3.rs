use serde::Deserialize;

// VJP3: Flow collections over many lines
// The file contains one failing example and one valid example. We implement the valid one:
// k: {\n k\n :\n v\n }

#[derive(Debug, Deserialize, PartialEq)]
struct Outer {
    k: Inner,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Inner {
    k: String,
}

#[test]
fn yaml_vjp3_flow_collections_multiline_valid() {
    // Valid case (second in the YAML file)
    let y = indoc::indoc!("k: {\n k\n :\n v\n }\n");

    let v: Outer = serde_saphyr::from_str(y).expect("failed to parse VJP3 valid case");
    assert_eq!(v.k.k, "v");
}
