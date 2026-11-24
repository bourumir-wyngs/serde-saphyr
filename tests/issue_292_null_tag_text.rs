#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{Error, from_reader, from_str, from_str_multiple};

#[derive(Debug, Deserialize, PartialEq)]
struct Holder<T> {
    v: T,
}

/// The YAML 1.2 core schema spells null as empty, `~`, `null`, `Null` or `NULL`.
#[rstest]
#[case::empty("v: !!null\n")]
#[case::tilde("v: !!null ~\n")]
#[case::lowercase("v: !!null null\n")]
#[case::capitalized("v: !!null Null\n")]
#[case::uppercase("v: !!null NULL\n")]
fn null_tag_reads_each_null_spelling(#[case] yaml: &str) {
    assert_eq!(from_str::<Holder<()>>(yaml).unwrap(), Holder { v: () });
    assert_eq!(
        from_str::<Holder<Option<String>>>(yaml).unwrap(),
        Holder { v: None }
    );
    assert_eq!(
        from_str::<Holder<Option<u32>>>(yaml).unwrap(),
        Holder { v: None }
    );
    assert_eq!(
        from_str::<Holder<serde_json::Value>>(yaml).unwrap(),
        Holder {
            v: serde_json::Value::Null
        }
    );
}

/// A `!!null` scalar whose text is not a null spelling is not a valid null.
#[rstest]
#[case::word("v: !!null foo\n")]
#[case::quoted_word("v: !!null \"foo\"\n")]
#[case::zero("v: !!null 0\n")]
#[case::boolean("v: !!null false\n")]
#[case::mixed_case("v: !!null nULL\n")]
fn null_tag_refuses_non_null_text(#[case] yaml: &str) {
    for result in [
        from_str::<Holder<()>>(yaml),
        from_reader::<_, Holder<()>>(yaml.as_bytes()),
    ] {
        let err = result.unwrap_err();
        assert!(
            matches!(err.without_snippet(), Error::UnexpectedValueForUnit { .. }),
            "{yaml:?}: {err}",
        );
    }
    assert!(
        from_str::<Holder<Option<String>>>(yaml).is_err(),
        "{yaml:?} must not read as None",
    );
    assert!(
        from_str::<Holder<Option<u32>>>(yaml).is_err(),
        "{yaml:?} must not read as None",
    );
    assert!(
        from_str::<Holder<serde_json::Value>>(yaml).is_err(),
        "{yaml:?} must not read as null",
    );
}

#[test]
fn null_tag_with_non_null_text_is_not_a_skipped_document() {
    assert!(from_str_multiple::<String, Vec<_>>("--- !!null foo\n--- kept\n").is_err());
    assert_eq!(
        from_str_multiple::<String, Vec<_>>("--- !!null null\n--- kept\n").unwrap(),
        vec!["kept".to_owned()],
    );
}
