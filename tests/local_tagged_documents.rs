#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::{Deserialize, de::DeserializeOwned};
use serde_saphyr::{Error, Options, options};
use std::io::Cursor;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Command {
    Null,
    Now,
}

#[derive(Debug, Deserialize, PartialEq)]
enum Payload {
    #[serde(rename = "null")]
    Null(Option<String>),
    #[serde(rename = "now")]
    Now,
}

const LOCAL_DOCUMENTS: &str = "--- !null\n--- !now\n";
const OPTIONAL_DOCUMENTS: &str = "--- !null\n--- !now\n--- !null payload\n";
const EMPTY_DOCUMENTS: &str =
    "---\n--- null\n--- ~\n--- !!null null\n--- !<tag:yaml.org,2002:null>\n--- now\n";

/// Collect the same input through every public string, bytes, and reader multi-document API.
fn documents<T: DeserializeOwned>(yaml: &str) -> [Result<Vec<T>, Error>; 10] {
    let mut reader = Cursor::new(yaml.as_bytes());
    let mut reader_with_options = Cursor::new(yaml.as_bytes());
    [
        serde_saphyr::from_multiple(yaml),
        serde_saphyr::from_multiple_with_options(yaml, Options::default()),
        serde_saphyr::from_str_multiple(yaml),
        serde_saphyr::from_str_multiple_with_options(yaml, Options::default()),
        serde_saphyr::from_slice_multiple(yaml.as_bytes()),
        serde_saphyr::from_slice_multiple_with_options(yaml.as_bytes(), Options::default()),
        serde_saphyr::from_bytes_multiple(yaml.as_bytes()),
        serde_saphyr::from_bytes_multiple_with_options(yaml.as_bytes(), Options::default()),
        serde_saphyr::read(&mut reader).collect(),
        serde_saphyr::read_with_options(&mut reader_with_options, Options::default()).collect(),
    ]
}

#[rstest]
#[case(LOCAL_DOCUMENTS, vec![Command::Null, Command::Now])]
#[case("!<!null>", vec![Command::Null])]
#[case("%TAG !local! !\n--- !local!null", vec![Command::Null])]
#[case("%TAG !! !\n--- !!null", vec![Command::Null])]
#[case(EMPTY_DOCUMENTS, vec![Command::Now])]
#[case("%TAG ! tag:yaml.org,2002:\n--- !null\n...\n--- now", vec![Command::Now])]
fn local_tagged_documents_are_retained(#[case] yaml: &str, #[case] expected: Vec<Command>) {
    for result in documents::<Command>(yaml) {
        assert_eq!(result.unwrap(), expected);
    }
}

#[test]
fn local_null_tag_keeps_the_enum_payload() {
    for result in documents::<Payload>("--- !null payload\n") {
        assert_eq!(result.unwrap(), [Payload::Null(Some("payload".to_owned()))]);
    }
}

#[test]
fn retained_documents_preserve_present_optional_values() {
    for result in documents::<Option<Payload>>(OPTIONAL_DOCUMENTS) {
        assert_eq!(
            result.unwrap(),
            [
                Some(Payload::Null(None)),
                Some(Payload::Now),
                Some(Payload::Null(Some("payload".into())))
            ]
        );
    }
}

#[test]
fn retained_custom_tags_still_obey_strict_tag_rejection() {
    let yaml = "--- !now\n";
    let options = options! { reject_unsupported_tags: true };
    let mut reader = Cursor::new(yaml.as_bytes());
    let results = [
        serde_saphyr::from_multiple_with_options::<Command>(yaml, options.clone()),
        serde_saphyr::from_slice_multiple_with_options::<Command>(yaml.as_bytes(), options.clone()),
        serde_saphyr::read_with_options::<_, Command>(&mut reader, options).collect(),
    ];
    for result in results {
        assert!(matches!(
            result.unwrap_err().without_snippet(),
            Error::UnsupportedTag { .. }
        ));
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
#[derive(Debug, Deserialize, PartialEq)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[serde(transparent)]
struct Validated<T>(#[cfg_attr(feature = "garde", garde(skip))] T);

#[cfg(feature = "validator")]
impl<T> validator::Validate for Validated<T> {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

#[cfg(feature = "garde")]
fn garde_documents<T: DeserializeOwned>(yaml: &str) -> [Result<Vec<Validated<T>>, Error>; 5] {
    let mut reader = Cursor::new(yaml.as_bytes());
    let mut reader_with_options = Cursor::new(yaml.as_bytes());
    [
        serde_saphyr::from_multiple_valid(yaml),
        serde_saphyr::from_multiple_with_options_valid(yaml, Options::default()),
        serde_saphyr::from_slice_multiple_with_options_valid(yaml.as_bytes(), Options::default()),
        serde_saphyr::read_valid(&mut reader).collect(),
        serde_saphyr::read_with_options_valid(&mut reader_with_options, Options::default())
            .collect(),
    ]
}

#[cfg(feature = "garde")]
#[rstest]
#[case(LOCAL_DOCUMENTS, vec![Command::Null, Command::Now])]
#[case(EMPTY_DOCUMENTS, vec![Command::Now])]
fn garde_preserves_local_tagged_documents(#[case] yaml: &str, #[case] expected: Vec<Command>) {
    for result in garde_documents::<Command>(yaml) {
        assert_eq!(
            result
                .unwrap()
                .into_iter()
                .map(|value| value.0)
                .collect::<Vec<_>>(),
            expected,
        );
    }
}

#[cfg(feature = "garde")]
#[test]
fn garde_preserves_present_optional_documents() {
    for result in garde_documents::<Payload>("--- !null payload\n") {
        assert_eq!(
            result.unwrap(),
            [Validated(Payload::Null(Some("payload".into())))]
        );
    }
    for result in garde_documents::<Option<Payload>>(OPTIONAL_DOCUMENTS) {
        assert_eq!(
            result.unwrap(),
            [
                Validated(Some(Payload::Null(None))),
                Validated(Some(Payload::Now)),
                Validated(Some(Payload::Null(Some("payload".into()))))
            ]
        );
    }
}

#[cfg(feature = "validator")]
fn validator_documents<T: DeserializeOwned>(yaml: &str) -> [Result<Vec<Validated<T>>, Error>; 5] {
    let mut reader = Cursor::new(yaml.as_bytes());
    let mut reader_with_options = Cursor::new(yaml.as_bytes());
    [
        serde_saphyr::from_multiple_validate(yaml),
        serde_saphyr::from_multiple_with_options_validate(yaml, Options::default()),
        serde_saphyr::from_slice_multiple_with_options_validate(
            yaml.as_bytes(),
            Options::default(),
        ),
        serde_saphyr::read_validate(&mut reader).collect(),
        serde_saphyr::read_with_options_validate(&mut reader_with_options, Options::default())
            .collect(),
    ]
}

#[cfg(feature = "validator")]
#[rstest]
#[case(LOCAL_DOCUMENTS, vec![Command::Null, Command::Now])]
#[case(EMPTY_DOCUMENTS, vec![Command::Now])]
fn validator_preserves_local_tagged_documents(#[case] yaml: &str, #[case] expected: Vec<Command>) {
    for result in validator_documents::<Command>(yaml) {
        assert_eq!(
            result
                .unwrap()
                .into_iter()
                .map(|value| value.0)
                .collect::<Vec<_>>(),
            expected,
        );
    }
}

#[cfg(feature = "validator")]
#[test]
fn validator_preserves_present_optional_documents() {
    for result in validator_documents::<Payload>("--- !null payload\n") {
        assert_eq!(
            result.unwrap(),
            [Validated(Payload::Null(Some("payload".into())))]
        );
    }
    for result in validator_documents::<Option<Payload>>(OPTIONAL_DOCUMENTS) {
        assert_eq!(
            result.unwrap(),
            [
                Validated(Some(Payload::Null(None))),
                Validated(Some(Payload::Now)),
                Validated(Some(Payload::Null(Some("payload".into()))))
            ]
        );
    }
}

#[rstest]
#[case("!<tag:example.org,2026:unit> null")]
#[case("%TAG !e! tag:example.org,2026:\n--- !e!unit")]
fn global_application_tags_are_not_skipped_as_null(#[case] yaml: &str) {
    for result in documents::<Option<()>>(yaml) {
        assert_eq!(result.unwrap(), [Some(())]);
    }
}

#[rstest]
#[case("!unknown")]
#[case("!unknown null")]
fn optional_document_errors_are_not_hidden_by_null_resolution(#[case] yaml: &str) {
    for result in documents::<Option<Command>>(yaml) {
        assert!(result.is_err());
    }
}
