#![cfg(all(feature = "serialize", feature = "deserialize"))]

#[test]
fn from_str_with_options() {
    let opts = serde_saphyr::options! {};
    let v: i32 = serde_saphyr::from_str_with_options("42", opts).unwrap();
    assert_eq!(v, 42);
}

#[test]
fn from_slice_basic() {
    let v: i32 = serde_saphyr::from_slice(b"42").unwrap();
    assert_eq!(v, 42);
}

#[test]
fn from_reader_basic() {
    let data = b"hello" as &[u8];
    let v: String = serde_saphyr::from_reader(data).unwrap();
    assert_eq!(v, "hello");
}
