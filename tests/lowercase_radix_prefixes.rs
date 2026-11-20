#![cfg(feature = "deserialize")]

use std::{borrow::Cow, collections::BTreeMap};

use rstest::rstest;
use serde_json::{Value, json};
use serde_saphyr::{
    DefaultMessageFormatter, DuplicateKeyPolicy, Error, MessageFormatter, Options,
    UserMessageFormatter, from_reader_with_options, from_str_with_options,
};

fn lowercase_only() -> Options {
    serde_saphyr::options! { lowercase_radix_prefixes_only: true }
}

#[track_caller]
fn assert_prefix_diagnostic(error: &Error) {
    assert!(matches!(
        error.without_snippet(),
        Error::UppercaseRadixPrefixDisabled { .. }
    ));
    assert_eq!(
        DefaultMessageFormatter.format_message(error),
        "Uppercase radix prefixes are disabled in configuration"
    );
    assert_eq!(
        UserMessageFormatter.format_message(error),
        "Uppercase radix prefixes not allowed"
    );
    assert!(
        error
            .render()
            .contains("Uppercase radix prefixes are disabled in configuration")
    );
    assert!(
        error
            .render_with_formatter(&UserMessageFormatter)
            .contains("Uppercase radix prefixes not allowed")
    );
}

#[rstest]
#[case::hex("0X1F")]
#[case::octal("0O37")]
#[case::binary("0B11111")]
#[case::positive("+0X1F")]
#[case::quoted("!!int '  +0X1F  '")]
#[case::tagged("!!int 0X1F")]
#[case::tagged_quoted("!!int '0X1F'")]
#[case::verbatim("!<tag:yaml.org,2002:int> 0X1F")]
fn integer_targets_share_prefix_policy(#[case] yaml: &str) {
    macro_rules! check {
        ($($ty:ty),+ $(,)?) => {$(
            let default: $ty = from_str_with_options(yaml, Options::default()).unwrap();
            assert_eq!(default, 31);
            assert_prefix_diagnostic(
                &from_str_with_options::<$ty>(yaml, lowercase_only()).unwrap_err(),
            );
            assert_prefix_diagnostic(
                &from_reader_with_options::<_, $ty>(yaml.as_bytes(), lowercase_only()).unwrap_err(),
            );
        )+};
    }
    check!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);
}

#[rstest]
#[case::hex("!!int -0X1F")]
#[case::octal("!!int -0O37")]
#[case::binary("!!int -0B11111")]
#[case::quoted("!!int ' +0X1F '")]
fn explicit_tags_cannot_override_prefix_policy(#[case] yaml: &str) {
    for error in [
        from_str_with_options::<Value>(yaml, lowercase_only()).unwrap_err(),
        from_reader_with_options::<_, Value>(yaml.as_bytes(), lowercase_only()).unwrap_err(),
        from_str_with_options::<i128>(yaml, lowercase_only()).unwrap_err(),
    ] {
        assert_prefix_diagnostic(&error);
    }
}

#[rstest]
#[case::missing_digits("0X")]
#[case::bad_hex("0XGG")]
#[case::bad_octal("0O8")]
#[case::bad_binary("0B2")]
#[case::bad_separator("0XF__F")]
#[case::lookalike("0Xnot_a_number")]
fn malformed_numbers_keep_parser_errors(#[case] scalar: &str) {
    for options in [Options::default(), lowercase_only()] {
        for error in [
            from_str_with_options::<i32>(scalar, options.clone()).unwrap_err(),
            from_reader_with_options::<_, i32>(scalar.as_bytes(), options).unwrap_err(),
        ] {
            assert!(matches!(
                error.without_snippet(),
                Error::InvalidScalar { ty: "i32", .. }
            ));
        }
    }
}

#[test]
fn range_and_negative_unsigned_failures_keep_parser_errors() {
    macro_rules! check {
        ($ty:ty, $yaml:literal) => {
            for options in [Options::default(), lowercase_only()] {
                for error in [
                    from_str_with_options::<$ty>($yaml, options.clone()).unwrap_err(),
                    from_reader_with_options::<_, $ty>($yaml.as_bytes(), options).unwrap_err(),
                ] {
                    assert!(matches!(
                        error.without_snippet(),
                        Error::InvalidScalar { ty, .. } if *ty == stringify!($ty)
                    ));
                }
            }
        };
    }
    check!(i8, "0X80");
    check!(u8, "0X100");
    check!(u32, "-0X1F");
    check!(i128, "0X80000000000000000000000000000000");
    check!(u128, "0X100000000000000000000000000000000");
}

#[test]
fn prefix_policy_preserves_value_and_key_error_locations() {
    for (yaml, line, column, offset) in [("0X1F: value\n", 1, 1, 0), ("value:\n  0X1F\n", 2, 3, 9)]
    {
        let errors = if line == 1 {
            [
                from_str_with_options::<BTreeMap<u32, String>>(yaml, lowercase_only()).unwrap_err(),
                from_reader_with_options::<_, BTreeMap<u32, String>>(
                    yaml.as_bytes(),
                    lowercase_only(),
                )
                .unwrap_err(),
            ]
        } else {
            [
                from_str_with_options::<BTreeMap<String, u32>>(yaml, lowercase_only()).unwrap_err(),
                from_reader_with_options::<_, BTreeMap<String, u32>>(
                    yaml.as_bytes(),
                    lowercase_only(),
                )
                .unwrap_err(),
            ]
        };
        for error in errors {
            assert_prefix_diagnostic(&error);
            let location = error.location().unwrap();
            assert_eq!((location.line(), location.column()), (line, column));
            assert_eq!(location.span().offset(), offset);
        }
    }
}

#[rstest]
#[case::hex("0X1F", "31")]
#[case::octal("0O37", "31")]
#[case::binary("0B11111", "31")]
#[case::negative("-0X1F", "-31")]
fn integer_key_identity_follows_prefix_policy(
    #[case] scalar: &str,
    #[case] decimal: &str,
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
        DuplicateKeyPolicy::PassThrough
    )]
    policy: DuplicateKeyPolicy,
) {
    let yaml = format!("{scalar}: text\n{decimal}: integer\n");
    let options = serde_saphyr::options! {
        lowercase_radix_prefixes_only: true,
        duplicate_keys: policy,
    };
    let expected = BTreeMap::from([
        (scalar.to_owned(), "text".to_owned()),
        (decimal.to_owned(), "integer".to_owned()),
    ]);
    for result in [
        from_str_with_options::<BTreeMap<String, String>>(&yaml, options.clone()),
        from_reader_with_options::<_, BTreeMap<String, String>>(yaml.as_bytes(), options.clone()),
    ] {
        assert_eq!(result.unwrap(), expected);
    }
    assert_prefix_diagnostic(
        &from_str_with_options::<BTreeMap<i64, String>>(&yaml, options).unwrap_err(),
    );
}

#[rstest]
#[case::explicit_after_merge("<<: {31: integer}\n0X1F: text\n")]
#[case::explicit_before_merge("0X1F: text\n<<: {31: integer}\n")]
#[case::merged_uppercase("<<: {0X1F: text}\n31: integer\n")]
#[case::separate_merge_sources("<<: [{31: integer}, {0X1F: text}]\n")]
fn merge_keys_share_prefix_policy(
    #[case] yaml: &str,
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
        DuplicateKeyPolicy::PassThrough
    )]
    policy: DuplicateKeyPolicy,
) {
    let options = serde_saphyr::options! {
        lowercase_radix_prefixes_only: true,
        duplicate_keys: policy,
    };
    let expected = BTreeMap::from([
        ("0X1F".to_owned(), "text".to_owned()),
        ("31".to_owned(), "integer".to_owned()),
    ]);
    for result in [
        from_str_with_options::<BTreeMap<String, String>>(yaml, options.clone()),
        from_reader_with_options::<_, BTreeMap<String, String>>(yaml.as_bytes(), options.clone()),
    ] {
        assert_eq!(result.unwrap(), expected);
    }
    assert_prefix_diagnostic(
        &from_str_with_options::<BTreeMap<i64, String>>(yaml, options).unwrap_err(),
    );
}

#[rstest]
#[case::sequence("? [31]\n: integer\n? [0X1F]\n: text\n")]
#[case::mapping_key("? [{31: same}]\n: integer\n? [{0X1F: same}]\n: text\n")]
#[case::mapping_value("? [{id: 31}]\n: integer\n? [{id: 0X1F}]\n: text\n")]
fn composite_key_identity_uses_shared_integer_policy(#[case] yaml: &str) {
    for error in [
        from_str_with_options::<serde::de::IgnoredAny>(yaml, Options::default()).unwrap_err(),
        from_reader_with_options::<_, serde::de::IgnoredAny>(yaml.as_bytes(), Options::default())
            .unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::DuplicateMappingKey { .. }
        ));
    }
    assert!(from_str_with_options::<serde::de::IgnoredAny>(yaml, lowercase_only()).is_ok());
    assert!(
        from_reader_with_options::<_, serde::de::IgnoredAny>(yaml.as_bytes(), lowercase_only())
            .is_ok()
    );
}

#[rstest]
#[case::hex("0X1F")]
#[case::octal("+0O37")]
#[case::binary("-0B11111")]
fn no_schema_accepts_restricted_prefixes_as_strings(#[case] scalar: &str) {
    let options = serde_saphyr::options! {
        lowercase_radix_prefixes_only: true,
        no_schema: true,
    };
    assert_eq!(
        from_str_with_options::<String>(scalar, options.clone()).unwrap(),
        scalar
    );
    assert_eq!(
        from_str_with_options::<&str>(scalar, options.clone()).unwrap(),
        scalar
    );
    assert_eq!(
        from_str_with_options::<Cow<'_, str>>(scalar, options.clone()).unwrap(),
        scalar
    );
    let yaml = format!("{scalar}: {scalar}\n");
    let map: BTreeMap<&str, &str> = from_str_with_options(&yaml, options).unwrap();
    assert_eq!(map, BTreeMap::from([(scalar, scalar)]));
}

#[rstest]
fn prefix_and_underscore_policies_are_independent(
    #[values(false, true)] lowercase_only: bool,
    #[values(false, true)] allow_underscores: bool,
    #[values(false, true)] legacy_octal: bool,
) {
    let options = serde_saphyr::options! {
        lowercase_radix_prefixes_only: lowercase_only,
        allow_numeric_underscores: allow_underscores,
        legacy_octal_numbers: legacy_octal,
    };
    for (scalar, allowed) in [
        ("0xFF", true),
        ("0XFF", !lowercase_only),
        ("0xF_F", allow_underscores),
        ("0XF_F", !lowercase_only && allow_underscores),
    ] {
        let typed = from_str_with_options::<u16>(scalar, options.clone());
        assert_eq!(typed.is_ok(), allowed, "{scalar}");
        let inferred: Value = from_str_with_options(scalar, options.clone()).unwrap();
        assert_eq!(inferred, if allowed { json!(255) } else { json!(scalar) });
    }
}

#[cfg(feature = "properties")]
#[rstest]
#[case::plain("${NUMBER}")]
#[case::tagged("!!int ${NUMBER}")]
fn interpolated_numbers_obey_prefix_policy(#[case] yaml: &str) {
    let options = lowercase_only().with_properties(std::collections::HashMap::from([(
        "NUMBER".to_owned(),
        "0X1F".to_owned(),
    )]));
    assert_prefix_diagnostic(&from_str_with_options::<u32>(yaml, options.clone()).unwrap_err());
    if yaml.starts_with("!!int") {
        assert_prefix_diagnostic(&from_str_with_options::<Value>(yaml, options).unwrap_err());
    } else {
        let value: Value = from_str_with_options(yaml, options).unwrap();
        assert_eq!(value, json!("0X1F"));
    }
}

#[test]
fn prefix_restriction_defaults_off_and_is_visible_in_debug() {
    assert!(!Options::default().lowercase_radix_prefixes_only);
    assert!(format!("{:?}", lowercase_only()).contains("lowercase_radix_prefixes_only: true"));
}

#[cfg(feature = "serde_derived_types")]
#[test]
fn serialized_prefix_option_round_trips_and_missing_field_defaults_false() {
    let mut value = serde_json::to_value(lowercase_only()).unwrap();
    assert_eq!(value["lowercase_radix_prefixes_only"], json!(true));
    let options: Options = serde_json::from_value(value.clone()).unwrap();
    assert!(options.lowercase_radix_prefixes_only);
    value
        .as_object_mut()
        .unwrap()
        .remove("lowercase_radix_prefixes_only");
    let options: Options = serde_json::from_value(value).unwrap();
    assert!(!options.lowercase_radix_prefixes_only);
}
