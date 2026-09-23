#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;
use std::collections::{BTreeSet, VecDeque};

fn unwrap_snippet(err: &serde_saphyr::Error) -> &serde_saphyr::Error {
    match err {
        serde_saphyr::Error::WithSnippet { error, .. } => error,
        other => other,
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct Person {
    name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct BorrowedPerson<'a> {
    name: &'a str,
}

fn assert_borrows_from(value: &str, input: &[u8]) {
    let input_start = input.as_ptr() as usize;
    let value_start = value.as_ptr() as usize;
    assert!(value_start >= input_start);
    assert!(value_start + value.len() <= input_start + input.len());
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
    let docs: Vec<Person> = serde_saphyr::from_str_multiple(y).expect("parse single doc as multi");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].name, "John");
}

#[test]
fn single_document_entrypoint_rejects_multiple_documents() {
    let y = "name: A\n---\nname: B\n";
    let err = serde_saphyr::from_str::<Person>(y).expect_err("expected multi-doc stream to fail");
    match unwrap_snippet(&err) {
        serde_saphyr::Error::MultipleDocuments { .. } => {}
        other => panic!("expected MultipleDocuments error, got {other:?}"),
    }
}

#[test]
fn multiple_documents_one_with_markers() {
    // Single document delimited by --- and ... markers
    let y = "---\nname: Jane\n...\n";
    let docs: Vec<Person> = serde_saphyr::from_str_multiple(y).expect("parse single doc delimited");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].name, "Jane");
}

#[test]
fn multiple_documents_two_documents() {
    // Two documents separated by ---
    let y = "name: A\n---\nname: B\n";
    let docs: Vec<Person> = serde_saphyr::from_str_multiple(y).expect("parse two docs");
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].name, "A");
    assert_eq!(docs[1].name, "B");
}

#[test]
fn str_multiple_documents_borrow_from_input_and_skip_empty_documents() {
    let yaml = String::from("\u{FEFF}---\n---\nname: First\n---\nnull\n---\nname: 'Second'\n");
    let docs: Vec<BorrowedPerson<'_>> = serde_saphyr::from_str_multiple(&yaml).unwrap();
    assert_eq!(
        docs,
        vec![
            BorrowedPerson { name: "First" },
            BorrowedPerson { name: "Second" },
        ]
    );
    for doc in docs {
        assert_borrows_from(doc.name, yaml.as_bytes());
    }
}

#[test]
fn str_multiple_documents_with_options_borrow_and_honor_duplicate_key_policy() {
    let yaml = String::from("name: First\nname: Ignored\n---\nname: Second\nname: Also ignored\n");
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::FirstWins,
    };
    let docs: Vec<BorrowedPerson<'_>> =
        serde_saphyr::from_str_multiple_with_options(&yaml, options).unwrap();
    assert_eq!(
        docs,
        vec![
            BorrowedPerson { name: "First" },
            BorrowedPerson { name: "Second" },
        ]
    );
    for doc in docs {
        assert_borrows_from(doc.name, yaml.as_bytes());
    }
}

#[test]
fn str_multiple_documents_reject_borrowing_transformed_strings() {
    let yaml = String::from("name: First\n---\nname: \"hello\\nworld\"\n");
    let err = serde_saphyr::from_str_multiple::<BorrowedPerson<'_>, Vec<_>>(&yaml).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        serde_saphyr::Error::CannotBorrowTransformedString { .. }
    ));
}

#[test]
#[allow(deprecated)]
fn str_multiple_documents_support_callback_adapters_and_owned_output() {
    // Forwarding closures let callbacks accept any input lifetime while returning owned values.
    let parse: for<'a> fn(&'a str) -> Result<Vec<String>, serde_saphyr::Error> =
        |input| serde_saphyr::from_str_multiple::<String, _>(input);
    let parse_with_options: for<'a> fn(
        &'a str,
        serde_saphyr::Options,
    ) -> Result<Vec<String>, serde_saphyr::Error> =
        |input, options| serde_saphyr::from_str_multiple_with_options::<String, _>(input, options);

    // Deprecated entrypoints still coerce directly to higher-ranked function pointers.
    let legacy_parse: for<'a> fn(&'a str) -> Result<Vec<String>, serde_saphyr::Error> =
        serde_saphyr::from_multiple::<String>;
    let legacy_parse_with_options: for<'a> fn(
        &'a str,
        serde_saphyr::Options,
    ) -> Result<Vec<String>, serde_saphyr::Error> =
        serde_saphyr::from_multiple_with_options::<String>;

    let results = {
        let yaml = String::from("First\n---\nSecond\n");
        [
            parse(&yaml).unwrap(),
            parse_with_options(&yaml, serde_saphyr::Options::default()).unwrap(),
            legacy_parse(&yaml).unwrap(),
            legacy_parse_with_options(&yaml, serde_saphyr::Options::default()).unwrap(),
            serde_saphyr::from_str_multiple::<String, Vec<_>>(&yaml).unwrap(),
            serde_saphyr::from_str_multiple_with_options::<String, Vec<_>>(
                &yaml,
                serde_saphyr::Options::default(),
            )
            .unwrap(),
        ]
    };
    for docs in results {
        assert_eq!(docs, ["First", "Second"]);
    }
}

#[test]
fn bytes_multiple_documents_borrow_from_input_and_skip_empty_documents() {
    let yaml = String::from("\u{FEFF}---\n---\nname: First\n---\nnull\n---\nname: 'Second'\n")
        .into_bytes();
    let docs: Vec<BorrowedPerson<'_>> = serde_saphyr::from_bytes_multiple(&yaml).unwrap();
    assert_eq!(
        docs,
        vec![
            BorrowedPerson { name: "First" },
            BorrowedPerson { name: "Second" },
        ]
    );
    for doc in docs {
        assert_borrows_from(doc.name, &yaml);
    }

    for empty in [b"".as_slice(), b"---\n...\n", b"null\n---\n~\n"] {
        assert!(
            serde_saphyr::from_bytes_multiple::<BorrowedPerson<'_>, Vec<_>>(empty)
                .unwrap()
                .is_empty()
        );
        assert!(
            serde_saphyr::from_bytes_multiple_with_options::<BorrowedPerson<'_>, Vec<_>>(
                empty,
                serde_saphyr::Options::default(),
            )
            .unwrap()
            .is_empty()
        );
    }
}

#[test]
fn bytes_multiple_documents_with_options_borrow_and_honor_duplicate_key_policy() {
    let yaml = String::from("name: First\nname: Ignored\n---\nname: Second\nname: Also ignored\n")
        .into_bytes();
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::FirstWins,
    };
    let docs: Vec<BorrowedPerson<'_>> =
        serde_saphyr::from_bytes_multiple_with_options(&yaml, options).unwrap();
    assert_eq!(
        docs,
        vec![
            BorrowedPerson { name: "First" },
            BorrowedPerson { name: "Second" },
        ]
    );
    for doc in docs {
        assert_borrows_from(doc.name, &yaml);
    }
}

#[test]
fn bytes_multiple_documents_reject_borrowing_transformed_strings() {
    let yaml = String::from("name: First\n---\nname: \"hello\\nworld\"\n").into_bytes();
    for result in [
        serde_saphyr::from_bytes_multiple::<BorrowedPerson<'_>, Vec<_>>(&yaml),
        serde_saphyr::from_bytes_multiple_with_options::<BorrowedPerson<'_>, Vec<_>>(
            &yaml,
            serde_saphyr::Options::default(),
        ),
    ] {
        assert!(matches!(
            result.unwrap_err().without_snippet(),
            serde_saphyr::Error::CannotBorrowTransformedString { .. }
        ));
    }
}

#[test]
fn bytes_multiple_documents_reject_invalid_utf8() {
    for result in [
        serde_saphyr::from_bytes_multiple::<Person, Vec<_>>(&[0xFF]),
        serde_saphyr::from_bytes_multiple_with_options::<Person, Vec<_>>(
            &[0xFF],
            serde_saphyr::Options::default(),
        ),
    ] {
        assert!(matches!(
            result.unwrap_err(),
            serde_saphyr::Error::InvalidUtf8Input
        ));
    }
}

#[test]
#[allow(deprecated)]
fn bytes_multiple_documents_support_callback_adapters_and_owned_output() {
    let parse: for<'a> fn(&'a [u8]) -> Result<Vec<String>, serde_saphyr::Error> =
        |input| serde_saphyr::from_bytes_multiple::<String, _>(input);
    let parse_with_options: for<'a> fn(
        &'a [u8],
        serde_saphyr::Options,
    ) -> Result<Vec<String>, serde_saphyr::Error> = |input, options| {
        serde_saphyr::from_bytes_multiple_with_options::<String, _>(input, options)
    };

    // Deprecated entrypoints still coerce directly to higher-ranked function pointers.
    let legacy_parse: for<'a> fn(&'a [u8]) -> Result<Vec<String>, serde_saphyr::Error> =
        serde_saphyr::from_slice_multiple::<String>;
    let legacy_parse_with_options: for<'a> fn(
        &'a [u8],
        serde_saphyr::Options,
    ) -> Result<Vec<String>, serde_saphyr::Error> =
        serde_saphyr::from_slice_multiple_with_options::<String>;

    let results = {
        let yaml = String::from("First\n---\nSecond\n").into_bytes();
        [
            parse(&yaml).unwrap(),
            parse_with_options(&yaml, serde_saphyr::Options::default()).unwrap(),
            legacy_parse(&yaml).unwrap(),
            legacy_parse_with_options(&yaml, serde_saphyr::Options::default()).unwrap(),
            serde_saphyr::from_bytes_multiple::<String, Vec<_>>(&yaml).unwrap(),
            serde_saphyr::from_bytes_multiple_with_options::<String, Vec<_>>(
                &yaml,
                serde_saphyr::Options::default(),
            )
            .unwrap(),
        ]
    };
    for docs in results {
        assert_eq!(docs, ["First", "Second"]);
    }
}

#[test]
fn multiple_documents_collect_into_vecdeque_in_document_order() {
    let yaml = "3\n---\n1\n---\n3\n";
    for docs in [
        serde_saphyr::from_str_multiple::<i32, VecDeque<_>>(yaml).unwrap(),
        serde_saphyr::from_str_multiple_with_options::<i32, VecDeque<_>>(
            yaml,
            serde_saphyr::Options::default(),
        )
        .unwrap(),
        serde_saphyr::from_bytes_multiple::<i32, VecDeque<_>>(yaml.as_bytes()).unwrap(),
        serde_saphyr::from_bytes_multiple_with_options::<i32, VecDeque<_>>(
            yaml.as_bytes(),
            serde_saphyr::Options::default(),
        )
        .unwrap(),
    ] {
        assert_eq!(docs, VecDeque::from([3, 1, 3]));
    }
}

#[test]
fn multiple_documents_collect_borrowed_strings_into_btreeset() {
    let yaml = String::from("Second\n---\nFirst\n---\nSecond\n");
    for docs in [
        serde_saphyr::from_str_multiple::<&str, BTreeSet<_>>(&yaml).unwrap(),
        serde_saphyr::from_str_multiple_with_options::<&str, BTreeSet<_>>(
            &yaml,
            serde_saphyr::Options::default(),
        )
        .unwrap(),
        serde_saphyr::from_bytes_multiple::<&str, BTreeSet<_>>(yaml.as_bytes()).unwrap(),
        serde_saphyr::from_bytes_multiple_with_options::<&str, BTreeSet<_>>(
            yaml.as_bytes(),
            serde_saphyr::Options::default(),
        )
        .unwrap(),
    ] {
        assert_eq!(docs, BTreeSet::from(["First", "Second"]));
        for value in docs {
            assert_borrows_from(value, yaml.as_bytes());
        }
    }
}

#[test]
fn multiple_documents_need_only_default_and_extend_for_output() {
    // This accumulator deliberately has no FromIterator or IntoIterator implementation.
    #[derive(Debug, Default, PartialEq)]
    struct Accumulator {
        count: usize,
        total: i32,
    }

    impl Extend<i32> for Accumulator {
        fn extend<I: IntoIterator<Item = i32>>(&mut self, iter: I) {
            for value in iter {
                self.count += 1;
                self.total += value;
            }
        }
    }

    for (yaml, expected) in [
        ("3\n---\n1\n---\n3\n", Accumulator { count: 3, total: 7 }),
        ("", Accumulator::default()),
        ("---\n...\n", Accumulator::default()),
        ("null\n---\n~\n", Accumulator::default()),
    ] {
        for result in [
            serde_saphyr::from_str_multiple::<i32, Accumulator>(yaml),
            serde_saphyr::from_str_multiple_with_options::<i32, Accumulator>(
                yaml,
                serde_saphyr::Options::default(),
            ),
            serde_saphyr::from_bytes_multiple::<i32, Accumulator>(yaml.as_bytes()),
            serde_saphyr::from_bytes_multiple_with_options::<i32, Accumulator>(
                yaml.as_bytes(),
                serde_saphyr::Options::default(),
            ),
        ] {
            assert_eq!(result.unwrap(), expected);
        }
    }
}

#[test]
fn multiple_documents_cross_document_anchor_error() {
    // Anchors must not leak across document boundaries.
    let y = "name: &a John\n---\nname: *a\n";
    let err = serde_saphyr::from_str_multiple::<Person, Vec<_>>(y)
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
    let docs1: Vec<Person> = serde_saphyr::from_str_multiple(y1).expect("parse empty doc 1");
    assert!(
        docs1.is_empty(),
        "expected empty vec for explicit empty document, got: {:?}",
        docs1
    );

    // Case 2: just document start without content
    let y2 = "---\n";
    let docs2: Vec<Person> = serde_saphyr::from_str_multiple(y2).expect("parse empty doc 2");
    assert!(
        docs2.is_empty(),
        "expected empty vec for start-only empty document, got: {:?}",
        docs2
    );

    // Case 3: multiple empties
    let y3 = "---\n---\n...\n";
    let docs3: Vec<Person> =
        serde_saphyr::from_str_multiple(y3).expect("parse multiple empty docs");
    assert!(
        docs3.is_empty(),
        "expected empty vec when only empty documents present, got: {:?}",
        docs3
    );

    // Case 4: completely empty stream
    let y4 = "";
    let docs4: Vec<Person> =
        serde_saphyr::from_str_multiple(y4).expect("parse completely empty stream");
    assert!(
        docs4.is_empty(),
        "expected empty vec for empty stream, got: {:?}",
        docs4
    );
}

#[test]
fn multiple_documents_preserve_quoted_null_like_scalars() {
    let y = "\"\"\n---\n\"~\"\n---\n\"null\"\n";
    let docs: Vec<String> =
        serde_saphyr::from_str_multiple(y).expect("parse quoted null-like docs");
    assert_eq!(docs, vec![String::new(), "~".to_owned(), "null".to_owned()]);
}

#[test]
fn reader_matches_from_str_multiple_for_tagged_null_like_scalars() {
    let y = "--- !!str null\n--- !!null not-null\n--- kept\n";
    let expected = vec!["null".to_owned(), "kept".to_owned()];

    let docs: Vec<String> = serde_saphyr::from_str_multiple(y).expect("parse tagged docs");
    assert_eq!(docs, expected);

    let mut reader = std::io::Cursor::new(y.as_bytes());
    let docs: Vec<String> = serde_saphyr::read::<_, String>(&mut reader)
        .map(|res| res.expect("streamed document should parse"))
        .collect();

    assert_eq!(docs, expected);
}

#[test]
fn multiple_documents_strips_bom_and_skips_plain_null_like_documents() {
    let y = "\u{FEFF}~\n---\nname: Bom\n---\nnull\n---\nname: Done\n";
    let docs: Vec<Person> = serde_saphyr::from_str_multiple(y).expect("parse documents with BOM");
    assert_eq!(
        docs,
        vec![
            Person {
                name: "Bom".to_owned(),
            },
            Person {
                name: "Done".to_owned(),
            },
        ]
    );
}

#[test]
#[allow(deprecated)]
fn from_slice_multiple_with_options_rejects_invalid_utf8() {
    let err = serde_saphyr::from_slice_multiple_with_options::<Person>(
        &[0xFF],
        serde_saphyr::options! {},
    )
    .expect_err("invalid UTF-8 input should fail");
    assert!(matches!(err, serde_saphyr::Error::InvalidUtf8Input));
}

#[test]
fn multiple_documents_enum_variants() {
    let y = "person:\n  name: Alice\n  age: 30\n---\npet:\n  kind: cat\n---\nperson:\n  name: Bob\n  age: 25\n";
    let docs: Vec<Document> = serde_saphyr::from_str_multiple(y).expect("parse enum documents");
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
#[test]
fn from_str_multiple_documents_error() {
    let yaml = "---\nhello\n---\nworld\n";
    let result: Result<String, _> = serde_saphyr::from_str(yaml);
    let err = result.unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        serde_saphyr::Error::MultipleDocuments { .. }
    ));
}

#[test]
fn read_multiple_documents() {
    let yaml = "---\nhello\n---\nworld\n";
    let docs: Vec<String> = serde_saphyr::from_str_multiple(yaml).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0], "hello");
    assert_eq!(docs[1], "world");
}
