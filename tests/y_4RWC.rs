// 4RWC: Trailing spaces after flow collection
#[test]
fn yaml_4rwc_trailing_spaces_after_flow_collection() {
    let y = "[1, 2, 3]  \n"; // trailing spaces before newline
    let v: Vec<i32> = serde_saphyr::from_str(y).expect("failed to parse 4RWC");
    assert_eq!(v, vec![1, 2, 3]);
}
