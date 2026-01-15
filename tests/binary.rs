use serde::Deserialize;

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

#[test]
fn test_serde_saphyr_binary_supporting() -> anyhow::Result<()> {
    let content = "name: !!binary H4sIAA==";

    #[derive(Deserialize)]
    struct SupportsBinary {
        name: Vec<u8>,
    }

    let value: SupportsBinary = serde_saphyr::from_str(content)?;
    assert_eq!(value.name, vec![31, 139, 8, 0]);

    Ok(())
}

#[test]
fn test_serde_saphyr_binary_supporting_false() -> anyhow::Result<()> {
    let content = "name: !!binary H4sIAA==";

    #[derive(Deserialize)]
    struct SupportsBinary {
        name: Vec<u8>,
    }

    let options = serde_saphyr::Options {
        ignore_binary_tag_for_string: false, // Still should be fine as the target is not string
        ..serde_saphyr::Options::default()
    };

    let value: SupportsBinary = serde_saphyr::from_str_with_options(content, options)?;
    assert_eq!(value.name, vec![31, 139, 8, 0]);

    Ok(())
}

#[test]
fn test_serde_saphyr_json_value() -> anyhow::Result<()> {
    let content = "name: !!binary H4sIAA==";
    let options = serde_saphyr::Options {
        ignore_binary_tag_for_string: true,
        ..serde_saphyr::Options::default()
    };

    let value: serde_json::Value = serde_saphyr::from_str_with_options(content, options)?;
    assert_eq!(value["name"], "H4sIAA==");
    Ok(())
}
