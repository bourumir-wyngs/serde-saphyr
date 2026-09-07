#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde_saphyr::{DoubleQuoted, FlowSeq, FoldStr, LitStr, SingleQuoted, from_str, to_string};
use std::collections::BTreeMap;

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
