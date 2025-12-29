use serde::Deserialize;

// W4TN: Spec Example 9.5. Directives Documents
// First document: %YAML directive, then a literal block scalar with trailing newline "%!PS-Adobe-2.0\n".
// Second document: explicit empty document.

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Doc {
    S(String),
    Null(Option<String>),
}

#[test]
fn yaml_w4tn_directives_documents() {
    let y = indoc::indoc!("%YAML 1.2\n--- |\n%!PS-Adobe-2.0\n...\n%YAML 1.2\n---\n# Empty\n...\n");

    let docs: Vec<Doc> = serde_saphyr::from_multiple(y).expect("failed to parse W4TN");

    // from_multiple may skip empty docs; so either 1 or 2 docs.
    assert!(
        docs.len() == 1 || docs.len() == 2,
        "unexpected docs: {:?}",
        docs
    );

    match &docs[0] {
        Doc::S(s) => assert_eq!(s, "%!PS-Adobe-2.0\n"),
        other => panic!("expected first doc to be a string, got: {:?}", other),
    }

    if docs.len() == 2 {
        match &docs[1] {
            Doc::Null(None) => {}
            other => panic!("expected second doc to be Null(None), got: {:?}", other),
        }
    }
}
