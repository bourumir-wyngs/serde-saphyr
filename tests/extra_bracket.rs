#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource};

// granit-parser 0.0.6 does not emit closing event.
#[test]
fn extra_bracket_should_err() {
    let y = "---\n[ a, b, c ] ]\n";
    let result: Error = serde_saphyr::from_str::<serde_json::Value>(y).expect_err("Expected error");
    assert!(matches!(
        result.without_snippet(),
        Error::ExternalMessage {
            source,
            ..
        } if matches!(
            source.as_ref(),
            ExternalMessageSource::Parser(error)
                if *error.kind() == ErrorKind::MisplacedFlowCollectionEnd
        )
    ));
}
