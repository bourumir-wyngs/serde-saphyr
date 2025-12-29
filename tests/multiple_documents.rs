use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Person {
    name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
enum Document {
    #[serde(rename = "person")]
    Person { name: String, age: u8 },
    #[serde(rename = "pet")]
    Pet { kind: String },
}

#[test]
fn multiple_documents_one_no_markers() {
    // Single document without any explicit --- or ... markers
    let y = "name: John\n";
    let docs: Vec<Person> = serde_saphyr::from_multiple(y).expect("parse single doc as multi");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].name, "John");
}

#[test]
fn multiple_documents_one_with_markers() {
    // Single document delimited by --- and ... markers
    let y = "---\nname: Jane\n...\n";
    let docs: Vec<Person> = serde_saphyr::from_multiple(y).expect("parse single doc delimited");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].name, "Jane");
}

#[test]
fn multiple_documents_two_documents() {
    // Two documents separated by ---
    let y = "name: A\n---\nname: B\n";
    let docs: Vec<Person> = serde_saphyr::from_multiple(y).expect("parse two docs");
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].name, "A");
    assert_eq!(docs[1].name, "B");
}

#[test]
fn multiple_documents_cross_document_anchor_error() {
    // Anchors must not leak across document boundaries.
    let y = "name: &a John\n---\nname: *a\n";
    let err = serde_saphyr::from_multiple::<Person>(y)
        .expect_err("expected cross-document alias to fail");
    match &err {
        serde_saphyr::Error::UnknownAnchor { .. } => {}
        serde_saphyr::Error::WithSnippet { error, .. }
            if matches!(error.as_ref(), serde_saphyr::Error::UnknownAnchor { .. }) => {}
        other => panic!("expected unknown anchor error, got {other:?}"),
    }
}

#[test]
fn multiple_documents_empty_document_cases() {
    // Case 1: explicitly empty document
    let y1 = "---\n...\n";
    let docs1: Vec<Person> = serde_saphyr::from_multiple(y1).expect("parse empty doc 1");
    assert!(
        docs1.is_empty(),
        "expected empty vec for explicit empty document, got: {:?}",
        docs1
    );

    // Case 2: just document start without content
    let y2 = "---\n";
    let docs2: Vec<Person> = serde_saphyr::from_multiple(y2).expect("parse empty doc 2");
    assert!(
        docs2.is_empty(),
        "expected empty vec for start-only empty document, got: {:?}",
        docs2
    );

    // Case 3: multiple empties
    let y3 = "---\n---\n...\n";
    let docs3: Vec<Person> = serde_saphyr::from_multiple(y3).expect("parse multiple empty docs");
    assert!(
        docs3.is_empty(),
        "expected empty vec when only empty documents present, got: {:?}",
        docs3
    );

    // Case 4: completely empty stream
    let y4 = "";
    let docs4: Vec<Person> =
        serde_saphyr::from_multiple(y4).expect("parse completely empty stream");
    assert!(
        docs4.is_empty(),
        "expected empty vec for empty stream, got: {:?}",
        docs4
    );
}

#[test]
fn multiple_documents_preserve_quoted_null_like_scalars() {
    let y = "\"\"\n---\n\"~\"\n---\n\"null\"\n";
    let docs: Vec<String> = serde_saphyr::from_multiple(y).expect("parse quoted null-like docs");
    assert_eq!(docs, vec![String::new(), "~".to_owned(), "null".to_owned()]);
}

#[test]
fn multiple_documents_enum_variants() {
    let y = "person:\n  name: Alice\n  age: 30\n---\npet:\n  kind: cat\n---\nperson:\n  name: Bob\n  age: 25\n";
    let docs: Vec<Document> = serde_saphyr::from_multiple(y).expect("parse enum documents");
    assert_eq!(
        docs,
        vec![
            Document::Person {
                name: "Alice".to_owned(),
                age: 30,
            },
            Document::Pet {
                kind: "cat".to_owned(),
            },
            Document::Person {
                name: "Bob".to_owned(),
                age: 25,
            },
        ],
    );
}
