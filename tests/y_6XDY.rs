// 6XDY: Two document start markers — empty documents may be skipped by from_multiple
// or represented as `None` when deserializing into Option<String>.
#[test]
fn yaml_6xdy_two_null_documents() {
    let y = "---\n---\n";
    let docs: Vec<Option<String>> = serde_saphyr::from_multiple(y).expect("failed to parse 6XDY");
    if docs.is_empty() {
        // Parser skipped empty documents — acceptable behavior per policy.
        return;
    }
    // Alternatively, parsers may keep empty docs as `None` when T = Option<_>.
    assert_eq!(docs.len(), 2, "Expected two entries for two empty docs or none at all, got: {:?}", docs);
    assert!(docs.iter().all(|d| d.is_none()), "Expected all entries to be None for empty docs, got: {:?}", docs);
}
