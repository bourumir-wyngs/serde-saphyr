// Q4CL: Trailing content after quoted value → should be a parse error
#[test]
fn yaml_q4cl_trailing_content_after_quoted_value_should_error() {
    let y = r#"key1: "quoted1"
key2: "quoted2" trailing content
key3: "quoted3"
"#;
    let res: Result<std::collections::BTreeMap<String, String>, _> = serde_saphyr::from_str(&y);
    assert!(res.is_err(), "Q4CL must fail to parse due to trailing content after quoted scalar");
}
