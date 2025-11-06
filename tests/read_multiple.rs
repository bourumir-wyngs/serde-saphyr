use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Simple { id: usize }

#[test]
fn read_multiple_documents_iterator_skips_null_and_reads_values() {
    // Multiple documents: valid, null-like (should be skipped), valid, empty (just separator), valid
    let yaml = "id: 1\n---\n~\n---\nid: 2\n---\n---\nid: 3\n";
    let reader = std::io::Cursor::new(yaml.as_bytes());

    let iter = serde_saphyr::read::<_, Simple>(reader);

    let values: Vec<Simple> = iter.map(|r| r.expect("unexpected error while reading docs"))
                                  .collect();

    assert_eq!(values.len(), 3, "iterator should skip null-like/empty documents");
    assert_eq!(values[0], Simple { id: 1 });
    assert_eq!(values[1], Simple { id: 2 });
    assert_eq!(values[2], Simple { id: 3 });
}

#[test]
fn read_with_options_iterator_works_the_same() {
    let yaml = "---\nid: 10\n---\nnull\n---\nid: 20\n";
    let reader = std::io::Cursor::new(yaml.as_bytes());

    let opts = serde_saphyr::Options::default();
    let iter = serde_saphyr::read_with_options::<_, Simple>(reader, opts);

    let values: Vec<Simple> = iter.map(|r| r.expect("unexpected error while reading docs"))
                                  .collect();

    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Simple { id: 10 });
    assert_eq!(values[1], Simple { id: 20 });
}
