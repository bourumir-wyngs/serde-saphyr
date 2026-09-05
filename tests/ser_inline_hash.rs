#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use rstest::rstest;
use serde_saphyr::{FlowMap, FlowSeq, from_str, to_string, to_string_with_options};

#[rstest]
#[case::inline("a#b", "a#b", "a#b")]
#[case::trailing("a#", "a#", "a#")]
#[case::repeated("a##b", "a##b", "a##b")]
#[case::unicode("é#界", "é#界", "é#界")]
#[case::url(
    "https://example.com/#section",
    "\"https://example.com/#section\"",
    "https://example.com/#section"
)]
#[case::leading("#start", "\"#start\"", "\"#start\"")]
#[case::space("a #b", "\"a #b\"", "\"a #b\"")]
#[case::tab("a\t#b", "\"a\\t#b\"", "\"a\\t#b\"")]
#[case::later_comment("a#b #c", "\"a#b #c\"", "\"a#b #c\"")]
// Preserve the conservative policy for Unicode whitespace before a hash.
#[case::nbsp("a\u{a0}#b", "\"a\u{a0}#b\"", "\"a\u{a0}#b\"")]
fn hashes_in_maps_and_sequences(
    #[case] input: &str,
    #[case] key: &str,
    #[case] value: &str,
    #[values(false, true)] flow: bool,
) {
    let map = BTreeMap::from([(input.to_owned(), input.to_owned())]);
    let sequence = vec![input.to_owned()];
    let (map_yaml, sequence_yaml, expected_map, expected_sequence) = if flow {
        (
            to_string(&FlowMap(&map)).unwrap(),
            to_string(&FlowSeq(&sequence)).unwrap(),
            format!("{{{key}: {value}}}\n"),
            format!("[{value}]\n"),
        )
    } else {
        (
            to_string(&map).unwrap(),
            to_string(&sequence).unwrap(),
            format!("{key}: {value}\n"),
            format!("- {value}\n"),
        )
    };

    assert_eq!(map_yaml, expected_map);
    assert_eq!(sequence_yaml, expected_sequence);
    assert_eq!(
        from_str::<BTreeMap<String, String>>(&map_yaml).unwrap(),
        map
    );
    assert_eq!(from_str::<Vec<String>>(&sequence_yaml).unwrap(), sequence);
}

#[rstest]
fn quote_all_quotes_inline_hash_values(#[values(false, true)] flow: bool) {
    let options = serde_saphyr::ser_options! { quote_all: true };
    let map = BTreeMap::from([("a#b".to_owned(), "a#b".to_owned())]);
    let yaml = if flow {
        to_string_with_options(&FlowMap(&map), options).unwrap()
    } else {
        to_string_with_options(&map, options).unwrap()
    };

    // quote_all applies to values; safe keys keep their ordinary plain style.
    assert_eq!(
        yaml,
        if flow {
            "{a#b: 'a#b'}\n"
        } else {
            "a#b: 'a#b'\n"
        }
    );
    assert_eq!(from_str::<BTreeMap<String, String>>(&yaml).unwrap(), map);
}

#[rstest]
fn long_inline_hash_values_can_auto_fold(#[values(false, true)] quote_all: bool) {
    let input = std::iter::repeat_n("word#fragment", 12)
        .collect::<Vec<_>>()
        .join(" ");
    let options = serde_saphyr::ser_options! { quote_all: quote_all };
    let yaml = to_string_with_options(&input, options).unwrap();

    if quote_all {
        assert_eq!(yaml, format!("'{input}'\n"));
    } else {
        assert!(yaml.starts_with(">-\n"), "expected folded scalar: {yaml}");
        assert!(yaml.lines().count() > 2, "expected wrapped lines: {yaml}");
    }
    assert_eq!(from_str::<String>(&yaml).unwrap(), input);
}

#[cfg(feature = "properties")]
#[rstest]
fn inline_hash_property_interpolation_depends_on_quoting(#[values(false, true)] quote_all: bool) {
    let input = "${NAME}#fragment";
    let yaml = to_string_with_options(&input, serde_saphyr::ser_options! { quote_all: quote_all })
        .unwrap();
    assert_eq!(
        yaml,
        if quote_all {
            "'${NAME}#fragment'\n"
        } else {
            "${NAME}#fragment\n"
        }
    );

    let properties = std::collections::HashMap::from([("NAME".to_owned(), "resolved".to_owned())]);
    let parsed: String = serde_saphyr::from_str_with_options(
        &yaml,
        serde_saphyr::options! {}.with_properties(properties),
    )
    .unwrap();
    assert_eq!(
        parsed,
        if quote_all {
            input
        } else {
            "resolved#fragment"
        }
    );
}
