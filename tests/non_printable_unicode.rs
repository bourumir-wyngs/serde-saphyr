#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde::Serialize;
use serde_saphyr::{
    DoubleQuoted, FlowMap, FlowSeq, FoldStr, LitStr, SingleQuoted, from_str, to_string,
};
use std::collections::BTreeMap;
use std::fmt;

#[rstest]
#[case::fffe('\u{FFFE}', "\"\\uFFFE\"\n")]
#[case::ffff('\u{FFFF}', "\"\\uFFFF\"\n")]
fn non_printable_strings_and_chars_are_escaped(#[case] ch: char, #[case] expected: &str) {
    let text = ch.to_string();
    let string_yaml = to_string(&text).unwrap();
    let char_yaml = to_string(&ch).unwrap();
    assert_eq!(
        (string_yaml.as_str(), char_yaml.as_str()),
        (expected, expected)
    );
    assert_eq!(from_str::<String>(&string_yaml).unwrap(), text);
    assert_eq!(from_str::<char>(&char_yaml).unwrap(), ch);
}

#[rstest]
fn quoting_modes_escape_non_printable_unicode(#[values('\u{FFFE}', '\u{FFFF}')] ch: char) {
    let text = ch.to_string();
    let options = serde_saphyr::ser_options! { quote_all: true };
    for yaml in [
        to_string(&DoubleQuoted(&text)).unwrap(),
        serde_saphyr::to_string_with_options(&text, options).unwrap(),
    ] {
        assert!(!yaml.contains(ch), "yaml must escape {ch:?}: {yaml:?}");
        assert_eq!(from_str::<String>(&yaml).unwrap(), text);
    }
}

#[rstest]
fn map_keys_and_values_escape_non_printable_unicode(#[values('\u{FFFE}', '\u{FFFF}')] ch: char) {
    let expected = BTreeMap::from([(format!("key{ch}"), format!("value{ch}"))]);
    let yaml = to_string(&expected).unwrap();
    assert!(!yaml.contains(ch), "yaml must escape {ch:?}: {yaml:?}");
    assert_eq!(
        from_str::<BTreeMap<String, String>>(&yaml).unwrap(),
        expected
    );
}

#[rstest]
fn flow_sequences_escape_non_printable_unicode(#[values('\u{FFFE}', '\u{FFFF}')] ch: char) {
    let expected = vec![ch.to_string(), format!("text{ch}")];
    let yaml = to_string(&FlowSeq(&expected)).unwrap();
    assert!(!yaml.contains(ch), "yaml must escape {ch:?}: {yaml:?}");
    assert_eq!(from_str::<Vec<String>>(&yaml).unwrap(), expected);
}

#[rstest]
fn block_styles_fall_back_to_escaped_strings(
    #[values('\u{FFFE}', '\u{FFFF}')] ch: char,
    #[values("auto", "literal", "folded")] style: &str,
) {
    let text = format!("first line\nsecond {ch} line\n");
    let yaml = match style {
        "auto" => to_string(&text),
        "literal" => to_string(&LitStr(&text)),
        "folded" => to_string(&FoldStr(&text)),
        _ => unreachable!(),
    }
    .unwrap();
    assert!(yaml.starts_with('"'), "expected escaped scalar: {yaml:?}");
    assert!(!yaml.contains(ch), "yaml must escape {ch:?}: {yaml:?}");
    assert_eq!(from_str::<String>(&yaml).unwrap(), text);
}

#[rstest]
fn single_quoted_rejects_non_printable_unicode(#[values('\u{FFFE}', '\u{FFFF}')] ch: char) {
    let error = to_string(&SingleQuoted(ch.to_string())).unwrap_err();
    assert!(matches!(
        error,
        serde_saphyr::SerializeError::SingleQuotedRequiresEscaping { ch: actual } if actual == ch
    ));
}

#[rstest]
fn permitted_unicode_stays_readable(#[values('\u{FFFD}', '\u{1FFFE}')] ch: char) {
    let text = ch.to_string();
    let yaml = to_string(&text).unwrap();
    assert_eq!(yaml, format!("{text}\n"));
    assert_eq!(from_str::<String>(&yaml).unwrap(), text);
}

#[test]
fn mixed_unicode_preserves_literal_escape_sequences_and_adjacent_characters() {
    let text = "é\u{FFFE}\u{FFFF}🦀\\uFFFE\\uFFFF\"\n\t\u{FFFD}\u{1FFFE}";
    let expected = "\"é\\uFFFE\\uFFFF🦀\\\\uFFFE\\\\uFFFF\\\"\\n\\t\u{FFFD}\u{1FFFE}\"\n";

    let yaml = to_string(&text).unwrap();
    assert_eq!(yaml, expected);
    assert_eq!(from_str::<String>(&yaml).unwrap(), text);
}

#[test]
fn flow_mapping_escapes_char_keys_and_values() {
    let expected = BTreeMap::from([('\u{FFFE}', '\u{FFFF}'), ('\u{FFFF}', '\u{FFFE}')]);
    let yaml = to_string(&FlowMap(&expected)).unwrap();

    assert_eq!(
        yaml,
        "{\"\\uFFFE\": \"\\uFFFF\", \"\\uFFFF\": \"\\uFFFE\"}\n"
    );
    assert_eq!(from_str::<BTreeMap<char, char>>(&yaml).unwrap(), expected);
}

#[rstest]
fn collect_str_escapes_non_printable_unicode_in_keys_and_values(#[values(false, true)] flow: bool) {
    #[derive(Eq, Ord, PartialEq, PartialOrd)]
    struct DisplayText(char);

    impl fmt::Display for DisplayText {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Separate writes exercise collect_str with characters inside a formatted value.
            f.write_str("before")?;
            write!(f, "{}", self.0)?;
            f.write_str("after")
        }
    }

    impl Serialize for DisplayText {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_str(self)
        }
    }

    let values = BTreeMap::from([(DisplayText('\u{FFFE}'), DisplayText('\u{FFFF}'))]);
    let yaml = if flow {
        to_string(&FlowMap(&values))
    } else {
        to_string(&values)
    }
    .unwrap();
    let entry = "\"before\\uFFFEafter\": \"before\\uFFFFafter\"";
    let expected_yaml = if flow {
        format!("{{{entry}}}\n")
    } else {
        format!("{entry}\n")
    };

    assert_eq!(yaml, expected_yaml);
    assert_eq!(
        from_str::<BTreeMap<String, String>>(&yaml).unwrap(),
        BTreeMap::from([(
            "before\u{FFFE}after".to_owned(),
            "before\u{FFFF}after".to_owned()
        )])
    );
}
