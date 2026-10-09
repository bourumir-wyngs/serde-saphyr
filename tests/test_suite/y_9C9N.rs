use granit_parser::ErrorKind;
use serde_json::Value;
use serde_saphyr::{Error, ExternalMessageSource};

fn assert_strict_indentation_error(yaml: &str) {
    let error = serde_saphyr::from_str_with_options::<Value>(
        yaml,
        serde_saphyr::options! { strict_indentation: true },
    )
    .expect_err("strict indentation must reject under-indented flow sequences");
    assert!(
        matches!(
            error.without_snippet(),
            Error::ExternalMessage { source, .. }
                if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                    if error.kind() == &ErrorKind::InvalidIndentation)
        ),
        "expected a parser indentation error for {yaml:?}, got {error}"
    );
}

// 9C9N: Wrong indented flow sequence — marked fail: true.
#[test]
fn yaml_9c9n_wrong_indented_flow_sequence_should_fail() {
    assert_strict_indentation_error("---\nflow: [a,\nb,\nc]\n");
}

#[test]
fn yaml_9c9n_flow_sequences_require_indentation_in_strict_mode() {
    assert!(!serde_saphyr::Options::default().strict_indentation);
    for (yaml, compliant) in [
        ("---\nflow: [a,\nb,\nc]\n", "---\nflow: [a,\n b,\n c]\n"),
        ("flow: [\na\n ]\n", "flow: [\n a\n ]\n"),
        ("flow: [\n'a'\n ]\n", "flow: [\n 'a'\n ]\n"),
        ("flow: [\n\"a\"\n ]\n", "flow: [\n \"a\"\n ]\n"),
        ("flow: [\n[nested]\n ]\n", "flow: [\n [nested]\n ]\n"),
        (
            "flow: [\n{nested: value}\n ]\n",
            "flow: [\n {nested: value}\n ]\n",
        ),
        ("flow: [\n a\n, b\n ]\n", "flow: [\n a\n , b\n ]\n"),
        ("flow: [first\nsecond]\n", "flow: [first\n second]\n"),
        ("flow: ['first\nsecond']\n", "flow: ['first\n second']\n"),
        (
            "flow: [\"first\nsecond\"]\n",
            "flow: [\"first\n second\"]\n",
        ),
        (
            "flow: [\"first\\\nsecond\"]\n",
            "flow: [\"first\\\n second\"]\n",
        ),
        (
            "outer:\n  flow: [\n  value\n   ]\n",
            "outer:\n  flow: [\n   value\n   ]\n",
        ),
        ("- flow: [\n  value\n   ]\n", "- flow: [\n   value\n   ]\n"),
        (
            "outer:\n  flow: [one\n  , two]\n",
            "outer:\n  flow: [one\n   , two]\n",
        ),
        ("outer:\n  flow: [\n  ]\n", "outer:\n  flow: [\n   ]\n"),
    ] {
        let expected: Value = serde_saphyr::from_str_with_options(
            compliant,
            serde_saphyr::options! { strict_indentation: true },
        )
        .unwrap_or_else(|error| panic!("strict mode rejected {compliant:?}: {error}"));
        let default: Value = serde_saphyr::from_str(yaml)
            .unwrap_or_else(|error| panic!("default mode rejected {yaml:?}: {error}"));
        let relaxed: Value = serde_saphyr::from_str_with_options(
            yaml,
            serde_saphyr::options! { strict_indentation: false },
        )
        .unwrap_or_else(|error| panic!("relaxed mode rejected {yaml:?}: {error}"));
        assert_eq!(default, expected, "input: {yaml:?}");
        assert_eq!(relaxed, expected, "input: {yaml:?}");
        assert_strict_indentation_error(yaml);
    }
}
