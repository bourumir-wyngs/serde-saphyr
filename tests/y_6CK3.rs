// 6CK3: Tag Shorthands — expect a sequence of strings
#[test]
fn yaml_6ck3_tag_shorthands() {
    let y = "%TAG !e! tag:example.com,2000:app/\n---\n- !local foo\n- !!str bar\n- !e!tag%21 baz\n";
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse 6CK3");
    assert_eq!(v, vec!["foo".to_string(), "bar".to_string(), "baz".to_string()]);
}
