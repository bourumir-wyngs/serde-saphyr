#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Serialize;
use serde_saphyr::to_string;

#[test]
fn bytes_serialized_as_base64_or_sequence() {
    struct Bytes<'a>(&'a [u8]);
    impl Serialize for Bytes<'_> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(self.0)
        }
    }
    let yaml = to_string(&Bytes(b"hello")).unwrap();
    // Just ensure it doesn't panic and produces some output
    assert!(!yaml.is_empty(), "yaml: {yaml}");
}

#[test]
fn serialize_bytes_inline_as_binary() {
    // serde_bytes makes the field call serialize_bytes in value position (mid-line)
    #[derive(Serialize)]
    struct B {
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    }
    let b = B {
        data: vec![1, 2, 3],
    };
    let yaml = to_string(&b).unwrap();
    assert!(yaml.contains("!!binary"), "expected !!binary tag: {yaml}");
}

#[test]
fn serialize_bytes_top_level_as_seq() {
    // Top-level &[u8] should serialize as a block sequence of integers
    let data: &[u8] = &[10, 20];
    let yaml = to_string(&serde_bytes::Bytes::new(data)).unwrap();
    // When at line start, bytes go through serialize_seq path
    // Actually top-level serde_bytes::Bytes calls serialize_bytes which checks at_line_start
    assert!(
        yaml.contains("10") && yaml.contains("20"),
        "expected byte values: {yaml}"
    );
}

#[test]
fn serialize_bytes() {
    use serde::Serializer;
    let mut buf = String::new();
    {
        let mut ser = serde_saphyr::ser::YamlSerializer::new(&mut buf);
        ser.serialize_bytes(b"hello").unwrap();
    }
    // Just verify it produced some output (base64-encoded)
    assert!(!buf.is_empty(), "bytes serialization produced: {buf}");
}
