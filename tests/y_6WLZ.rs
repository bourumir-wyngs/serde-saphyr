// 6WLZ: Primary Tag Handle — two documents that both deserialize to "bar"
#[test]
fn yaml_6wlz_primary_tag_handle_two_docs() {
    let y = "# Private\n---\n!foo \"bar\"\n...\n# Global\n%TAG ! tag:example.com,2000:app/\n---\n!foo \"bar\"\n";
    let docs: Vec<String> = serde_saphyr::from_multiple(y).expect("failed to parse 6WLZ");
    assert_eq!(docs, vec!["bar".to_string(), "bar".to_string()]);
}
