#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde_saphyr::{FlowMap, from_str, ser_options, to_string, to_string_with_options};
use std::collections::BTreeMap;

#[rstest]
#[case::consecutive("1__0")]
#[case::trailing("1_")]
#[case::signed("-1__0")]
#[case::binary("0b_10_")]
#[case::octal("07__1_")]
#[case::hexadecimal("0x_0A_74_AE")]
#[case::exponent("1e_2")]
#[case::fractional("1._0")]
#[case::overflow("18446744073709551616_")]
#[case::sexagesimal_integer("10:20")]
#[case::sexagesimal_underscore("1_0:20")]
#[case::sexagesimal_repeated_underscore("1__0_:20")]
#[case::sexagesimal_negative("-10:20")]
#[case::sexagesimal_positive("+10:20")]
#[case::sexagesimal_multiple_groups("190:20:30")]
#[case::sexagesimal_overflow("18446744073709551616:59")]
#[case::sexagesimal_float("10:20.5")]
#[case::sexagesimal_float_underscore("1_0:20.5_0")]
#[case::sexagesimal_float_zero("0:20.5")]
#[case::sexagesimal_float_leading_zero("01:20.5")]
#[case::sexagesimal_float_empty_fraction("-1:59.")]
#[case::sexagesimal_float_underscore_fraction("1:59._")]
fn yaml11_numeric_strings_are_quoted_in_keys_and_values(#[case] text: &str) {
    let map = BTreeMap::from([(text, text)]);
    let scalar = to_string(&text).unwrap();
    let block = to_string(&map).unwrap();
    let flow = to_string(&FlowMap(&map)).unwrap();
    for (yaml, count) in [(&scalar, 1), (&block, 2), (&flow, 2)] {
        // Quoting, rather than only a same-library roundtrip, protects readers
        // whose YAML 1.1 resolver interprets the plain spelling as a number.
        let quoted = [format!("\"{text}\""), format!("'{text}'")]
            .iter()
            .map(|spelling| yaml.matches(spelling.as_str()).count())
            .sum::<usize>();
        assert_eq!(quoted, count, "{yaml}");
    }
    assert_eq!(from_str::<String>(&scalar).unwrap(), text);
    for yaml in [block, flow] {
        assert_eq!(
            from_str::<BTreeMap<String, String>>(&yaml).unwrap(),
            BTreeMap::from([(text.to_owned(), text.to_owned())])
        );
    }
}

#[test]
fn yaml12_keeps_non_numeric_underscore_spellings_plain() {
    let yaml = to_string_with_options(
        &BTreeMap::from([("1__0", "1_")]),
        ser_options! { yaml_12: true },
    )
    .unwrap();
    assert!(yaml.contains("1__0: 1_"), "{yaml}");
}

#[test]
fn ordinary_underscore_strings_stay_plain() {
    let yaml = to_string(&BTreeMap::from([("app_name", "build_id")])).unwrap();
    assert_eq!(yaml, "app_name: build_id\n");
}

#[rstest]
#[case("10:20")]
#[case("1_0:20")]
#[case("10:20.5")]
#[case("1_0:20.5_0")]
fn yaml12_keeps_sexagesimal_strings_plain(#[case] text: &str) {
    let yaml = to_string_with_options(&text, ser_options! { yaml_12: true }).unwrap();
    assert!(yaml.lines().any(|line| line == text), "{yaml}");
}

#[rstest]
#[case("0:20")]
#[case("01:20")]
#[case("1:60")]
#[case("1:000")]
#[case("1:2_0")]
#[case("1::20")]
#[case("1:20:60")]
#[case("1:60.5")]
#[case("1:20.5e2")]
#[case("1:20.5.0")]
#[case("app:build")]
fn non_numeric_colon_strings_stay_plain(#[case] text: &str) {
    assert_eq!(to_string(&text).unwrap(), format!("{text}\n"));
}
