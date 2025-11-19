use serde::{Deserialize, Serialize};
use serde_saphyr as yaml;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Data {
    data: Vec<u8>,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct DataBinary {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

#[test]
fn test_deserialize_binary_tag() {
    let yaml_str = "data: !!binary aGVsbG8=";
    let parsed: Data = yaml::from_str(yaml_str).unwrap();
    assert_eq!(parsed.data, b"hello");
}

#[test]
fn test_deserialize_as_array() {
    let yaml_str = "data: [104, 101, 108, 108, 111]";
    let parsed: Data = yaml::from_str(yaml_str).unwrap();
    assert_eq!(parsed.data, b"hello");
}

#[test]
fn test_deserialize_vec_u8_direct() {
    let bytes: Vec<u8> = yaml::from_str("!!binary AQID").unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn test_serialize_vec_as_sequence() {
    let data = Data {
        data: b"hi".to_vec(),
    };
    let yaml_str = yaml::to_string(&data).unwrap();
    assert_eq!(yaml_str, "data:\n  - 104\n  - 105\n");
}

#[test]
fn test_serialize_vec_as_bytes() {
    let data = DataBinary {
        data: b"hello".to_vec(),
    };
    let yaml_str = yaml::to_string(&data).unwrap();
    assert_eq!(yaml_str, "data: !!binary aGVsbG8=\n");
}
