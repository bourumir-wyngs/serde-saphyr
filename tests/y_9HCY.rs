use serde_json::Value;

// 9HCY: Need document footer before directives
// YAML places a directive (%TAG) after content of the first document but before a document
// end marker. Per the spec, directives are only allowed in the document header; a footer
// ("...") is required before directives can appear again. The test expects a parse error.

#[test]
fn yaml_9hcy_need_document_footer_before_directives() {
    let y = indoc::indoc!(
        "!foo \"bar\"\n%TAG ! tag:example.com,2000:app/\n---\n!foo \"bar\"\n"
    );

    let result: Result<Vec<Value>, serde_saphyr::Error> = serde_saphyr::from_multiple(y);

    assert!(
        result.is_err(),
        "Expected parser to reject directives after content without a document footer (9HCY)"
    );
}
