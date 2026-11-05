#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::Error;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{self, Read};

type Map = HashMap<String, String>;
const EMPTY_INPUTS: &[&str] = &["", " \n\r\n", "# comment", "\u{feff}# comment\n"];

#[derive(Debug, Default, Deserialize, PartialEq)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
struct Config {
    #[serde(default)]
    #[cfg_attr(feature = "garde", garde(skip))]
    entries: Map,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
struct EmptyTuple();

fn assert_empty_entrypoints<T>(input: &str)
where
    T: serde::de::DeserializeOwned + Default + PartialEq + Debug,
{
    let results = [
        serde_saphyr::from_str::<T>(input),
        serde_saphyr::from_slice::<T>(input.as_bytes()),
        serde_saphyr::from_reader::<_, T>(input.as_bytes()),
        serde_saphyr::with_deserializer_from_str(input, |de| T::deserialize(de)),
        serde_saphyr::with_deserializer_from_slice(input.as_bytes(), |de| T::deserialize(de)),
        serde_saphyr::with_deserializer_from_reader(input.as_bytes(), |de| T::deserialize(de)),
    ];
    for result in results {
        assert_eq!(result.unwrap(), T::default(), "input {input:?}");
    }
}

#[test]
fn empty_streams_restore_empty_collections_and_defaulted_structs() {
    for input in EMPTY_INPUTS {
        assert_empty_entrypoints::<Map>(input);
        assert_empty_entrypoints::<Vec<String>>(input);
        assert_empty_entrypoints::<Config>(input);
        assert_empty_entrypoints::<[String; 0]>(input);
        assert_empty_entrypoints::<EmptyTuple>(input);
    }
    let borrowed: HashMap<&str, &str> = serde_saphyr::from_str("").unwrap();
    assert!(borrowed.is_empty());
}

#[test]
fn empty_streams_preserve_scalar_and_nullable_behavior() {
    for input in EMPTY_INPUTS {
        assert_empty_entrypoints::<Option<Map>>(input);
        assert_empty_entrypoints::<()>(input);
        assert_eq!(
            serde_saphyr::from_str::<serde_json::Value>(input).unwrap(),
            serde_json::Value::Null
        );
        assert!(matches!(
            serde_saphyr::from_str::<bool>(input)
                .unwrap_err()
                .without_snippet(),
            Error::Eof { .. }
        ));
    }
}

#[test]
fn empty_streams_still_require_mandatory_fields() {
    #[derive(Debug, Deserialize)]
    struct Required {
        #[serde(rename = "name")]
        _name: String,
    }
    assert!(serde_saphyr::from_str::<Required>("").is_err());
}

#[test]
fn explicit_null_documents_still_reject_collection_targets() {
    for input in ["---", "---\n...", "null", "~", "!!null", "# comment\n---\n"] {
        assert!(serde_saphyr::from_str::<Map>(input).is_err(), "{input:?}");
        assert!(
            serde_saphyr::from_str::<Vec<String>>(input).is_err(),
            "{input:?}"
        );
        assert!(
            serde_saphyr::from_str::<Config>(input).is_err(),
            "{input:?}"
        );
    }
    assert!(serde_saphyr::from_str::<Map>("{}").unwrap().is_empty());
    assert!(
        serde_saphyr::from_str::<Vec<String>>("[]")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn empty_streams_do_not_add_documents_to_multi_document_results() {
    for input in EMPTY_INPUTS {
        assert!(
            serde_saphyr::from_multiple::<Map>(input)
                .unwrap()
                .is_empty()
        );
        assert!(
            serde_saphyr::from_slice_multiple::<Map>(input.as_bytes())
                .unwrap()
                .is_empty()
        );
        let mut reader = input.as_bytes();
        assert!(serde_saphyr::read::<_, Map>(&mut reader).next().is_none());
    }
}

#[test]
fn malformed_input_is_never_treated_as_an_empty_stream() {
    for input in ["{", "[", "%YAML 1.2\n", "---\n[", "{}\n...\n["] {
        assert!(serde_saphyr::from_str::<Map>(input).is_err(), "{input:?}");
        assert!(
            serde_saphyr::from_reader::<_, Map>(input.as_bytes()).is_err(),
            "{input:?}"
        );
    }
}

#[test]
fn reader_errors_are_never_treated_as_empty_streams() {
    struct FailingReader;
    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "reader failed",
            ))
        }
    }
    let error = serde_saphyr::from_reader::<_, Map>(FailingReader).unwrap_err();
    match error.without_snippet() {
        Error::IOError { cause } => assert_eq!(cause.kind(), io::ErrorKind::ConnectionReset),
        other => panic!("expected original reader error, got {other:?}"),
    }
}

#[test]
fn empty_streams_still_enforce_parser_budgets() {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_events: 0,
        },
    };
    assert!(serde_saphyr::from_str_with_options::<Map>("", options).is_err());
}

#[cfg(any(feature = "garde", feature = "validator"))]
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
struct InvalidDefault {
    #[serde(default)]
    #[cfg_attr(feature = "garde", garde(length(min = 1)))]
    #[cfg_attr(feature = "validator", validate(length(min = 1)))]
    name: String,
}

#[cfg(feature = "garde")]
#[test]
fn garde_validates_defaults_from_empty_streams() {
    assert_eq!(
        serde_saphyr::from_str_valid::<Config>("").unwrap(),
        Config::default()
    );
    assert_eq!(
        serde_saphyr::from_reader_valid::<_, Config>(&b""[..]).unwrap(),
        Config::default()
    );
    let error = serde_saphyr::from_str_valid::<InvalidDefault>("").unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::ValidationError { .. }
    ));
}

#[cfg(feature = "validator")]
#[test]
fn validator_validates_defaults_from_empty_streams() {
    assert_eq!(
        serde_saphyr::from_str_validate::<Config>("").unwrap(),
        Config::default()
    );
    assert_eq!(
        serde_saphyr::from_reader_validate::<_, Config>(&b""[..]).unwrap(),
        Config::default()
    );
    let error = serde_saphyr::from_str_validate::<InvalidDefault>("").unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::ValidationError { .. }
    ));
}
