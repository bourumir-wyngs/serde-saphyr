#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{Error, from_reader, from_str};

#[derive(Debug, Deserialize, PartialEq)]
struct Unit;

#[derive(Debug, Deserialize, PartialEq)]
struct Holder {
    unit: Unit,
}

#[rstest]
#[case::empty_flow_mapping("{}\n")]
#[case::flow_mapping("{a: 1}\n")]
#[case::block_mapping("a: 1\n")]
#[case::tagged_mapping("!!map {}\n")]
fn unit_struct_refuses_a_mapping(#[case] yaml: &str) {
    for result in [
        from_str::<Unit>(yaml),
        from_reader::<_, Unit>(yaml.as_bytes()),
    ] {
        assert!(
            matches!(
                result.unwrap_err().without_snippet(),
                Error::UnexpectedValueForUnit { .. }
            ),
            "{yaml:?} must not deserialize as a unit struct",
        );
    }
}

#[rstest]
#[case::empty_flow_mapping("unit: {}\n")]
#[case::block_mapping("unit:\n  a: 1\n")]
fn unit_struct_field_refuses_a_mapping(#[case] yaml: &str) {
    let err = from_str::<Holder>(yaml).unwrap_err();
    assert!(
        matches!(err.without_snippet(), Error::UnexpectedValueForUnit { .. }),
        "{yaml:?}: {err}",
    );
}

#[rstest]
#[case::null("unit: null\n")]
#[case::tilde("unit: ~\n")]
#[case::empty("unit:\n")]
#[case::null_tag("unit: !!null null\n")]
fn unit_struct_field_reads_null_forms(#[case] yaml: &str) {
    assert_eq!(from_str::<Holder>(yaml).unwrap(), Holder { unit: Unit });
}
