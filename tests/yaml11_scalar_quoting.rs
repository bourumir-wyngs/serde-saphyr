#![cfg(all(feature = "serialize", feature = "deserialize"))]

use proptest::prelude::*;
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
#[case::empty_binary("0b_")]
#[case::empty_hexadecimal("-0x__")]
#[case::go_binary_sign("0b+1")]
#[case::go_binary_negative("0b-1")]
#[case::go_binary_sign_underscore("0b_+1")]
#[case::float_empty_mantissa(".")]
#[case::float_multiple_dots("1.2.3")]
#[case::boolean("YES")]
#[case::short_boolean("n")]
#[case::null("Null")]
#[case::tilde("~")]
#[case::empty("")]
#[case::date("2026-09-09")]
#[case::invalid_date("2026-99-99")]
#[case::go_short_date("2026-9-9")]
#[case::timestamp("2026-09-09T12:34:56Z")]
#[case::timestamp_short_fields("2026-9-9t1:34:56.123+2:30")]
#[case::go_short_time("2026-9-9T1:2:3Z")]
#[case::go_comma_fraction("2026-09-09T12:34:56,123Z")]
#[case::timestamp_space("2026-09-09 12:34:56. -5")]
#[case::timestamp_tab("2026-09-09\t12:34:56\tZ")]
#[case::merge("<<")]
#[case::value("=")]
fn yaml11_implicit_strings_are_quoted_in_keys_and_values(#[case] text: &str) {
    assert_quoted_in_keys_and_values(text);
}

fn assert_quoted_in_keys_and_values(text: &str) {
    let map = BTreeMap::from([(text, text)]);
    let scalar = to_string(&text).unwrap();
    let block = to_string(&map).unwrap();
    let flow = to_string(&FlowMap(&map)).unwrap();
    for (yaml, count) in [(&scalar, 1), (&block, 2), (&flow, 2)] {
        // Quoting protects against other readers' implicit tags and constructor
        // errors, even when a same-library roundtrip would preserve the string.
        let quoted = [serde_json::to_string(text).unwrap(), format!("'{text}'")]
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

#[rstest]
#[case("0b_")]
#[case("1.2.3")]
#[case("2026-09-09")]
#[case("2026-09-09T12:34:56Z")]
#[case("<<")]
#[case("=")]
fn yaml12_keeps_yaml11_implicit_strings_plain(#[case] text: &str) {
    let yaml = to_string_with_options(&text, ser_options! { yaml_12: true }).unwrap();
    assert!(yaml.lines().any(|line| line == text), "{yaml}");
}

#[rstest]
#[case("build-2026-09-09")]
#[case("2026-09-09-release")]
#[case("2026-09-09T12:34")]
#[case("2026-09-09T12:34:56Zsuffix")]
#[case("2026-09-09T12:34:56+2:x")]
#[case("0b2")]
#[case("0xG")]
fn ordinary_scalar_strings_stay_plain(#[case] text: &str) {
    assert_eq!(to_string(&text).unwrap(), format!("{text}\n"));
}

// Independent reference grammars from https://yaml.org/type/{int,float,timestamp}.html.
// Generate syntax, not numeric/date values: overflow and construction errors are
// also failures to preserve strings. The decimal float alternatives include the
// fractional underscores accepted by YAML 1.1 readers such as PyYAML.
fn implicit_scalar() -> impl Strategy<Value = String> {
    prop_oneof![
        "[-+]?(0b[01_]+|0[0-7_]+|0|[1-9][0-9_]*|0x[0-9a-fA-F_]+|[1-9][0-9_]*(:[0-5]?[0-9])+)",
        r"[-+]?([0-9][0-9_]*)?\.[0-9.]*([eE][-+][0-9]+)?",
        r"[-+]?[0-9][0-9_]*(:[0-5]?[0-9])+\.[0-9_]*",
        r"[-+]?[0-9][0-9_]*\.[0-9_]*([eE][-+][0-9]+)?",
        r"[-+]?\.(inf|Inf|INF)|\.(nan|NaN|NAN)",
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}",
        r"[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}([Tt]|[ \t]+)[0-9]{1,2}:[0-9]{2}:[0-9]{2}(\.[0-9]*)?(([ \t]*)Z|[-+][0-9]{1,2}(:[0-9]{2})?)?",
        "y|Y|yes|Yes|YES|n|N|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF",
        "~|null|Null|NULL||<<|=",
    ]
}

proptest! {
    #[test]
    fn yaml11_implicit_grammar_strings_preserve_their_type(text in implicit_scalar()) {
        assert_quoted_in_keys_and_values(&text);
    }
}
