// 5TYM: Local Tag Prefix with two documents
// Expect two documents with tagged scalars; we deserialize into Vec<String> using from_multiple.
#[test]
fn yaml_5tym_local_tag_prefix_two_docs() {
    let y = "%TAG !m! !my-\n--- # Bulb here\n!m!light fluorescent\n...\n%TAG !m! !my-\n--- # Color here\n!m!light green\n";
    let docs: Vec<String> = serde_saphyr::from_multiple(y).expect("failed to parse 5TYM");
    assert_eq!(docs, vec!["fluorescent".to_string(), "green".to_string()]);
}
