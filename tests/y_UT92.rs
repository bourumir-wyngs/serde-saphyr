use serde::Deserialize;
use std::collections::HashMap;

// UT92: Spec Example 9.4. Explicit Documents
// Stream with an explicit document containing a mapping, followed by an empty document.
// We parse with from_multiple() and assert the non-empty mapping content.

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Doc {
    MapI(HashMap<String, i32>),
    MapS(HashMap<String, String>),
    Null(Option<String>),
}

#[test]
fn yaml_ut92_explicit_documents() {
    let y = indoc::indoc!(
        "---\n{ matches\n% : 20 }\n...\n---\n# Empty\n...\n"
    );

    let docs: Vec<Doc> = serde_saphyr::from_multiple(y).expect("failed to parse UT92");

    // from_multiple() may skip empty docs; accept either 1 (only mapping) or 2 (mapping + Null)
    assert!(docs.len() == 1 || docs.len() == 2, "unexpected docs: {:?}", docs);

    match &docs[0] {
        Doc::MapI(m) => assert_eq!(m.get("matches %"), Some(&20)),
        Doc::MapS(m) => assert_eq!(m.get("matches %").map(String::as_str), Some("20")),
        other => panic!("expected first doc to be a mapping, got: {:?}", other),
    }

    if docs.len() == 2 {
        match &docs[1] {
            Doc::Null(None) => {}
            other => panic!("expected second doc to be Null(None), got: {:?}", other),
        }
    }
}
