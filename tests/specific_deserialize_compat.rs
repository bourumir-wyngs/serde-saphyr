//! Migrating legacy flags into Specific must not change deserialization.
#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;
use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_saphyr::{Options, from_str_with_options, options, specific};

#[allow(deprecated)] // Compare the old public configuration with its replacement.
fn for_each_configuration(mut check: impl FnMut(&Options, &Options)) {
    for strict_booleans in [false, true] {
        for legacy_octal_numbers in [false, true] {
            for no_schema in [false, true] {
                let legacy = options! {
                    strict_booleans: strict_booleans,
                    legacy_octal_numbers: legacy_octal_numbers,
                    no_schema: no_schema,
                };
                for quote_all in [false, true] {
                    for yaml_12_quoting in [false, true] {
                        let specific = options! {
                            schema: specific! {
                                strict_booleans: strict_booleans,
                                legacy_octal_numbers: legacy_octal_numbers,
                                quote_all: quote_all,
                                yaml_12_quoting: yaml_12_quoting,
                            },
                            // Explicit Specific must also override conflicting old flags.
                            strict_booleans: !strict_booleans,
                            legacy_octal_numbers: !legacy_octal_numbers,
                            no_schema: no_schema,
                        };
                        check(&legacy, &specific);
                    }
                }
            }
        }
    }
}

#[track_caller]
fn assert_same<T>(yaml: &str, legacy: &Options, specific: &Options)
where
    T: DeserializeOwned + Debug + PartialEq,
{
    let parse = |options: &Options| {
        from_str_with_options::<T>(yaml, options.clone()).map_err(|error| error.to_string())
    };
    assert_eq!(
        parse(legacy),
        parse(specific),
        "input {yaml:?}, target {}, options {specific:?}",
        std::any::type_name::<T>(),
    );
}

#[test]
fn specific_matches_legacy_typed_and_inferred_scalars_including_errors() {
    for_each_configuration(|legacy, specific| {
        for yaml in [
            "true",
            "TrUe",
            "yes",
            "OFF",
            "n",
            "null",
            "~",
            "42",
            "-42",
            "010",
            "0_10",
            "0x_10",
            "0b_10",
            "0o_10",
            "0x10",
            "0b10",
            "1_000",
            "1.5",
            "1e2",
            ".inf",
            ".nan",
            "inf",
            "2001-12-15",
            "1:20",
            "word",
            "!!bool yes",
            "!!int 0x_10",
            "!!float 1e2",
            "!!null null",
            "!!str true",
            "'yes'",
            "\"010\"",
            "!!int invalid",
            "340282366920938463463374607431768211456",
        ] {
            assert_same::<Value>(yaml, legacy, specific);
            assert_same::<bool>(yaml, legacy, specific);
            assert_same::<i128>(yaml, legacy, specific);
            assert_same::<u128>(yaml, legacy, specific);
            assert_same::<String>(yaml, legacy, specific);
            assert_same::<Option<String>>(yaml, legacy, specific);
        }
    });
}

#[test]
fn specific_matches_legacy_float_values_and_conversion_errors() {
    for_each_configuration(|legacy, specific| {
        for yaml in [
            "1.5",
            "1e2",
            "1e-2",
            "1e+2",
            ".inf",
            "-.inf",
            ".nan",
            "1e9999",
            "0x_10",
            "word",
            "!!float word",
        ] {
            // Comparing the bits also handles NaN, which is not equal to itself.
            let parse = |options: &Options| {
                from_str_with_options::<f64>(yaml, options.clone())
                    .map(f64::to_bits)
                    .map_err(|error| error.to_string())
            };
            assert_eq!(parse(legacy), parse(specific), "{yaml:?}, {specific:?}");
        }
    });
}

#[test]
fn specific_matches_legacy_string_keys_characters_and_quoted_values() {
    for_each_configuration(|legacy, specific| {
        for yaml in ["true", "yes", "42", "010", "0_10", "0x_10", "word"] {
            assert_same::<BTreeMap<String, String>>(&format!("{yaml}: {yaml}\n"), legacy, specific);
            assert_same::<BTreeMap<String, String>>(
                &format!("'{yaml}': \"{yaml}\"\n"),
                legacy,
                specific,
            );
        }
        for yaml in ["y", "n", "1", "~", "x", "'1'", "!!str 1"] {
            assert_same::<char>(yaml, legacy, specific);
        }
    });
}

#[test]
fn specific_no_schema_preserves_the_historical_octal_exception() {
    let options = options! {
        schema: specific! { legacy_octal_numbers: true },
        no_schema: true,
    };
    for (yaml, expected_number) in [("0_10", 8), ("0x_10", 16)] {
        assert_eq!(
            from_str_with_options::<String>(yaml, options.clone()).unwrap(),
            yaml,
        );
        assert_eq!(
            from_str_with_options::<i64>(yaml, options.clone()).unwrap(),
            expected_number,
        );
    }
    assert!(from_str_with_options::<String>("yes", options).is_err());
    let strict = options! {
        schema: specific! { strict_booleans: true, legacy_octal_numbers: true },
        no_schema: true,
    };
    assert_eq!(
        from_str_with_options::<String>("yes", strict).unwrap(),
        "yes",
    );
}
