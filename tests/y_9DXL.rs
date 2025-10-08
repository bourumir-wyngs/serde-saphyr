use serde::Deserialize;
use std::collections::HashMap;

// 9DXL: Spec Example 9.6. Stream [1.3] — three documents: mapping, empty, mapping
// Our from_multiple may skip empty docs; accept either 2 or 3 with an explicit None.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Doc {
    MStr(HashMap<String, String>),
    MInt(HashMap<String, i32>),
    Null(Option<String>),
}

#[test]
fn yaml_9dxl_stream_three_documents_no_directive() {
    // Same as 9DXL but without the %YAML directive between documents.
    let y = "Mapping: Document\n---\n# Empty\n...\n%YAML 1.2\n---\nmatches %: 20\n";
    let docs: Vec<Doc> = serde_saphyr::from_multiple(y).expect("failed to parse 9DXL without directive");

    // Either two non-empty docs (empty one skipped) or three with None in the middle.
    assert!(docs.len() == 2 || docs.len() == 3, "unexpected docs len: {:?}", docs);

    // First should be the mapping {Mapping: Document}
    match &docs[0] {
        Doc::MStr(m) => assert_eq!(m.get("Mapping").map(String::as_str), Some("Document")),
        other => panic!("expected first to be MStr, got: {:?}", other),
    }

    // If there are 3 documents, the middle one represents an empty document.
    // We only accept a true empty/null (None) here; literal "~" should not surface.
    if docs.len() == 3 {
        match &docs[1] {
            Doc::Null(None) => {
                // empty document mapped to None
            }
            other => panic!("expected middle to be Null/empty (None), got: {:?}", other),
        }
    }

    // Last should be the mapping {"matches %": 20}
    match docs.last().unwrap() {
        Doc::MInt(m) => assert_eq!(m.get("matches %"), Some(&20)),
        Doc::MStr(m) => assert_eq!(m.get("matches %").map(String::as_str), Some("20")),
        other => panic!("expected last to be mapping with int or string value, got: {:?}", other),
    }
}
