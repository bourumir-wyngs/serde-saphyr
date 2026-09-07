#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;
use serde_bytes::ByteBuf;
use std::collections::BTreeMap;

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

    let options = serde_saphyr::options! {
        ignore_binary_tag_for_string: false, // Still should be fine as the target is not string
    };

    let value: SupportsBinary = serde_saphyr::from_str_with_options(content, options)?;
    assert_eq!(value.name, vec![31, 139, 8, 0]);

    Ok(())
}

#[test]
fn test_serde_saphyr_json_value() -> anyhow::Result<()> {
    let content = "name: !!binary H4sIAA==";
    let options = serde_saphyr::options! {
        ignore_binary_tag_for_string: true,
    };

    let value: serde_json::Value = serde_saphyr::from_str_with_options(content, options)?;
    assert_eq!(value["name"], "H4sIAA==");
    Ok(())
}

#[test]
fn binary_tag_rejects_padding_inside_quad() {
    #[derive(Deserialize)]
    struct SupportsBinary {
        #[allow(dead_code)]
        name: Vec<u8>,
    }

    let Err(err) = serde_saphyr::from_str::<SupportsBinary>("name: !!binary AA=A\n") else {
        panic!("padding is only valid at the end of a base64 quantum");
    };

    assert!(matches!(
        err.without_snippet(),
        serde_saphyr::Error::InvalidBinaryBase64 { .. }
    ));
}

#[rstest::rstest]
fn optional_binary_values_preserve_null_like_base64(
    #[values("!!binary", "!binary", "!<tag:yaml.org,2002:binary>")] tag: &str,
    #[values("null", "Null", "NULL", "")] payload: &str,
) {
    let yaml = format!("{tag} {payload}");
    let direct: ByteBuf = serde_saphyr::from_str(&yaml).unwrap();
    let optional: Option<ByteBuf> = serde_saphyr::from_str(&yaml).unwrap();

    if payload == "null" {
        assert_eq!(direct.as_ref(), [0x9e, 0xe9, 0x65]);
    } else if payload.is_empty() {
        assert!(direct.is_empty());
    }
    assert_eq!(optional, Some(direct), "{yaml}");
}

#[test]
fn optional_binary_rejects_malformed_base64_and_preserves_real_null() {
    let error = serde_saphyr::from_str::<Option<ByteBuf>>("!!binary ~").unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        serde_saphyr::Error::InvalidBinaryBase64 { .. }
    ));

    for yaml in ["null", "~", "---\n", "!!null null"] {
        assert_eq!(
            serde_saphyr::from_str::<Option<ByteBuf>>(yaml).unwrap(),
            None
        );
    }
}

#[test]
fn optional_binary_fields_and_buffered_values_preserve_decoded_bytes() {
    #[derive(Deserialize)]
    struct OptionalBinary {
        value: Option<ByteBuf>,
    }

    let yaml = "%TAG !bytes! tag:yaml.org,2002:\n---\nvalue: !bytes!binary null\n";
    let document: OptionalBinary = serde_saphyr::from_str(yaml).unwrap();
    let expected = ByteBuf::from(vec![0x9e, 0xe9, 0x65]);
    assert_eq!(document.value, Some(expected.clone()));

    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::LastWins,
    };
    let buffered: BTreeMap<u32, Option<ByteBuf>> =
        serde_saphyr::from_str_with_options("1: !!binary null\n2: !!binary\n", options).unwrap();
    assert_eq!(
        buffered,
        BTreeMap::from([(1, Some(expected)), (2, Some(ByteBuf::new()))])
    );
}

#[test]
fn document_streams_preserve_null_like_binary_values() {
    let yaml = "--- !!binary null\n--- null\n--- !!binary\n--- !!binary AQID\n";
    let expected = vec![
        ByteBuf::from(vec![0x9e, 0xe9, 0x65]),
        ByteBuf::new(),
        ByteBuf::from(vec![1, 2, 3]),
    ];
    let multiple: Vec<ByteBuf> = serde_saphyr::from_multiple(yaml).unwrap();
    let mut reader = yaml.as_bytes();
    let streamed = serde_saphyr::read::<_, ByteBuf>(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!((multiple, streamed), (expected.clone(), expected));
}

#[test]
fn document_streams_reject_malformed_binary_values() {
    let yaml = "--- !!binary ~\n";
    let multiple_error = serde_saphyr::from_multiple::<ByteBuf>(yaml).unwrap_err();
    let mut reader = yaml.as_bytes();
    let streamed_error = serde_saphyr::read::<_, ByteBuf>(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_err();

    for error in [multiple_error, streamed_error] {
        assert!(matches!(
            error.without_snippet(),
            serde_saphyr::Error::InvalidBinaryBase64 { .. }
        ));
    }
}
