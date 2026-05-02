#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Simple {
    id: usize,
}

#[test]
fn read_multiple_documents_iterator_skips_null_and_reads_values() {
    // Multiple documents: valid, null-like (should be skipped), valid, empty (just separator), valid
    let yaml = "id: 1\n---\n~\n---\nid: 2\n---\n---\nid: 3\n";
    let mut reader = std::io::Cursor::new(yaml.as_bytes());

    let iter = serde_saphyr::read::<_, Simple>(&mut reader);

    let values: Vec<Simple> = iter
        .map(|r| r.expect("unexpected error while reading docs"))
        .collect();

    assert_eq!(
        values.len(),
        3,
        "iterator should skip null-like/empty documents"
    );
    assert_eq!(values[0], Simple { id: 1 });
    assert_eq!(values[1], Simple { id: 2 });
    assert_eq!(values[2], Simple { id: 3 });
}

#[test]
fn read_with_options_iterator_works_the_same() {
    let yaml = "---\nid: 10\n---\nnull\n---\nid: 20\n";
    let mut reader = std::io::Cursor::new(yaml.as_bytes());

    let opts = serde_saphyr::options! {};
    let iter = serde_saphyr::read_with_options::<_, Simple>(&mut reader, opts);

    let values: Vec<Simple> = iter
        .map(|r| r.expect("unexpected error while reading docs"))
        .collect();

    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Simple { id: 10 });
    assert_eq!(values[1], Simple { id: 20 });
}

#[test]
fn from_reader_reports_empty_input_as_eof_for_non_null_target() {
    let err = serde_saphyr::from_reader::<_, bool>(std::io::Cursor::new(Vec::<u8>::new()))
        .expect_err("empty reader should not deserialize into bool");
    let message = err.to_string();
    assert!(
        message.contains("end") || message.contains("EOF") || message.contains("eof"),
        "expected EOF-like error, got: {message}"
    );
}

#[test]
fn from_reader_with_options_can_report_eof_without_snippet() {
    let options = serde_saphyr::options! {
        with_snippet: false,
    };
    let err = serde_saphyr::from_reader_with_options::<_, bool>(
        std::io::Cursor::new(Vec::<u8>::new()),
        options,
    )
    .expect_err("empty reader should not deserialize into bool");
    assert!(!matches!(err, serde_saphyr::Error::WithSnippet { .. }));
}

#[test]
fn from_reader_rejects_multiple_documents() {
    let yaml = b"id: 1\n---\nid: 2\n";
    let err = serde_saphyr::from_reader::<_, Simple>(std::io::Cursor::new(&yaml[..]))
        .expect_err("single-document reader API should reject multi-document input");
    let message = err.to_string();
    assert!(
        message.contains("multiple") || message.contains("document"),
        "expected multiple-document error, got: {message}"
    );
}

#[test]
fn from_reader_ignores_garbage_after_document_end_marker() {
    let yaml = b"id: 7\n...\n@ this is ignored after the document end\n";
    let value: Simple = serde_saphyr::from_reader(std::io::Cursor::new(&yaml[..]))
        .expect("garbage after explicit document end should be ignored");
    assert_eq!(value, Simple { id: 7 });
}

#[test]
fn from_reader_rejects_trailing_garbage_without_document_end_marker() {
    let yaml = b"id: 7\n@\n";
    let err = serde_saphyr::from_reader::<_, Simple>(std::io::Cursor::new(&yaml[..]))
        .expect_err("trailing garbage without document end should fail");
    assert!(!err.to_string().is_empty());
}

#[test]
fn read_iterator_returns_syntax_error_and_then_ends() {
    let yaml = b"id: 1\n---\n[\n";
    let mut reader = std::io::Cursor::new(&yaml[..]);
    let mut iter = serde_saphyr::read::<_, Simple>(&mut reader);

    assert_eq!(iter.next().unwrap().unwrap(), Simple { id: 1 });
    assert!(iter.next().unwrap().is_err());
    assert!(iter.next().is_none());
}
