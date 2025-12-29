use std::collections::BTreeMap;

// PUW8: Document start on last line — second document is empty
#[test]
fn yaml_puw8_document_start_on_last_line() {
    let y = r#"---
a: b
---
"#;
    let docs: Vec<BTreeMap<String, String>> =
        serde_saphyr::from_multiple(&y).expect("failed to parse PUW8 as multiple documents");
    // Only the first document has content; the trailing '---' alone represents an empty doc which should be skipped.
    assert_eq!(
        docs.len(),
        1,
        "expected only one non-empty document, got {docs:?}"
    );
    let doc = &docs[0];
    assert_eq!(doc.get("a").map(String::as_str), Some("b"));
}
