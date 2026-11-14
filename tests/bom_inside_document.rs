#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;

use serde_saphyr::{Options, from_reader_with_options, from_str_with_options};

type Map = BTreeMap<String, String>;

fn bom_as_content() -> Options {
    serde_saphyr::options! { bom_inside_document_as_content: true }
}

fn read(yaml: &str, options: Options) -> [Result<Map, serde_saphyr::Error>; 2] {
    [
        from_str_with_options(yaml, options.clone()),
        from_reader_with_options(yaml.as_bytes(), options),
    ]
}

fn map(entries: &[(&str, &str)]) -> Map {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

const IN_DOCUMENT: [(&str, &[(&str, &str)]); 3] = [
    ("a: 1\n\u{feff}b: 2\n", &[("a", "1"), ("\u{feff}b", "2")]),
    ("---\n\u{feff}a: 1\n", &[("\u{feff}a", "1")]),
    ("a: \u{feff}value\n", &[("a", "\u{feff}value")]),
];

#[test]
fn bom_inside_document_is_rejected_by_default() {
    assert!(!Options::default().bom_inside_document_as_content);
    for (yaml, _) in IN_DOCUMENT {
        for result in read(yaml, Options::default()) {
            assert!(result.is_err(), "accepted {yaml:?}");
        }
    }
}

#[test]
fn bom_inside_document_can_be_read_as_content() {
    for (yaml, expected) in IN_DOCUMENT {
        for result in read(yaml, bom_as_content()) {
            assert_eq!(result.unwrap(), map(expected), "{yaml:?}");
        }
    }
}

#[test]
fn leading_bom_is_skipped_in_both_modes() {
    for options in [Options::default(), bom_as_content()] {
        for result in read("\u{feff}a: 1\n", options) {
            assert_eq!(result.unwrap(), map(&[("a", "1")]));
        }
    }
}
