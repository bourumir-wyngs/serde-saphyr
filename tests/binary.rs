use serde::Deserialize;
use serde_saphyr;

#[derive(Debug, Deserialize, PartialEq)]
struct StructureWithBinaries {
    binary_form: Vec<u8>,
    array_form: Vec<u8>,
}

#[test]
fn bytes_via_binary_tag_and_array() {
    // "AQID" base64 → [1, 2, 3]
    let y1 = r#"
binary_form: !!binary AQID
array_form: [0, 127, 255]
"#;
    let v1: StructureWithBinaries = serde_saphyr::from_str(y1).unwrap();
    assert_eq!(v1.binary_form, vec![1, 2, 3]);
    assert_eq!(v1.array_form, vec![0, 127, 255]);

    // Multi-line block scalar "SGVsbG8h" → b"Hello!"
    let y2 = r#"
binary_form: !!binary |
  SGVs
  bG8h
array_form: [72, 101, 108, 108, 111, 33]
"#;
    let v2: StructureWithBinaries = serde_saphyr::from_str(y2).unwrap();
    assert_eq!(v2.binary_form, b"Hello!");
    assert_eq!(v2.array_form, b"Hello!");
}
