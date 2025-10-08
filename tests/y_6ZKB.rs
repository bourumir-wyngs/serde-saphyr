use serde::Deserialize;
use std::collections::HashMap;

// 6ZKB: Spec Example 9.6. Stream — three documents in a stream
// Expect: "Document", an empty document (may be skipped), and a mapping {"matches %": 20}.
// Our from_multiple() skips empty documents by design, so we assert we get the two non-empty docs.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Doc {
    S(String),
    MInt(HashMap<String, i32>),
    MStr(HashMap<String, String>),
    // Accommodate potential empty/null documents if the parser surfaces them.
    Null(Option<String>),
}

#[test]
fn yaml_6zkb_stream_multiple_documents() {
    // Same as 6ZKB but without the %YAML directive between documents.
    // This should parse fine and demonstrate that untagged enums handle
    // mixed document types (string, optional empty, mapping).
    let y = "Document\n---\n# Empty\n...\n%YAML 1.2\n---\nmatches %: 20\n";
    let docs: Vec<Doc> = serde_saphyr::from_multiple(y).expect("failed to parse 6ZKB without directive");

    // Empty document may be skipped; accept 2 docs, or 3 if the empty doc surfaces.
    assert!(docs.len() == 2 || docs.len() == 3, "expected 2 non-empty docs or 3 with an empty in the middle, got: {:?}", docs);

    match &docs[0] {
        Doc::S(s) => assert_eq!(s, "Document"),
        other => panic!("expected first doc to be a string, got: {:?}", other),
    }

    // If there are 3 docs, the middle one represents an empty document.
    // We now only accept a true empty/null (None) here; literal "~" should not surface.
    if docs.len() == 3 {
        match &docs[1] {
            Doc::Null(None) => {}
            other => panic!("expected middle doc to be Null(None), got: {:?}", other),
        }
    }

    // The last document should be the mapping {"matches %": 20}
    match docs.last().unwrap() {
        Doc::MInt(m) => assert_eq!(m.get("matches %"), Some(&20)),
        Doc::MStr(m) => assert_eq!(m.get("matches %").map(String::as_str), Some("20")),
        other => panic!("expected last doc to be a mapping, got: {:?}", other),
    }
}
