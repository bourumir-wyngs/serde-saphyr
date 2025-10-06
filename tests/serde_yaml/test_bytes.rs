use serde_saphyr as yaml;

#[test]
fn test_serialize_bytes() {
    let bytes: &[u8] = &[1, 2, 3];
    let s = yaml::to_string(&bytes).unwrap();
    assert_eq!(s, "- 1\n- 2\n- 3\n");
}

#[test]
fn test_deserialize_byte_sequence() {
    let yaml = "- 1\n- 2\n- 3\n";
    let bytes: Vec<u8> = yaml::from_str(yaml).unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}
