// M7A3: Bare Documents — multi-document stream with a bare scalar and a block scalar
// Expect two non-empty documents: "Bare document" and a block scalar with one line

#[test]
fn yaml_m7a3_bare_documents() {
    let y = r#"Bare
document
...
# No document
...
|
%!PS-Adobe-2.0 # Not the first line
"#;

    let docs: Vec<String> = serde_saphyr::from_multiple(y).expect("failed to parse M7A3");
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].as_str(), "Bare document");
    assert_eq!(docs[1].as_str(), "%!PS-Adobe-2.0 # Not the first line\n");
}
