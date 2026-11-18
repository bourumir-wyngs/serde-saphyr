#![cfg(feature = "deserialize")]

use std::{borrow::Cow, collections::BTreeMap, fmt::Debug};

use rstest::rstest;
use serde::{Deserialize, de::DeserializeOwned};
use serde_saphyr::{Error, Options, from_reader_with_options, from_str_with_options};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
enum Example {
    Unit,
    Map(()),
}

#[track_caller]
fn assert_quoting_value(error: Error, expected: &str) {
    let error = error.without_snippet();
    assert!(
        matches!(error, Error::QuotingRequired { value, .. } if value == expected),
        "unexpected diagnostic: {error:?}"
    );
    assert!(
        error
            .to_string()
            .contains(&format!("The string value [{expected}] must be quoted"))
    );
}

/// Deserialize `yaml` with `options`; `expected` is the offending scalar text.
/// Returns nothing; checks diagnostics from both string and reader inputs.
#[track_caller]
fn assert_owned_error<T: DeserializeOwned + Debug>(yaml: &str, expected: &str, options: Options) {
    assert_quoting_value(
        from_str_with_options::<T>(yaml, options.clone()).unwrap_err(),
        expected,
    );
    assert_quoting_value(
        from_reader_with_options::<_, T>(yaml.as_bytes(), options).unwrap_err(),
        expected,
    );
}

#[track_caller]
fn assert_scalar_diagnostics(scalar: &str, options: Options) {
    assert_owned_error::<String>(scalar, scalar, options.clone());
    assert_owned_error::<Cow<'_, str>>(scalar, scalar, options.clone());
    assert_owned_error::<char>(scalar, scalar, options.clone());
    assert_owned_error::<Example>(scalar, scalar, options.clone());
    assert_quoting_value(
        from_str_with_options::<&str>(scalar, options.clone()).unwrap_err(),
        scalar,
    );
    let mapping = format!("{scalar}: null\n");
    assert_owned_error::<Example>(&mapping, scalar, options.clone());
    assert_owned_error::<BTreeMap<String, ()>>(&mapping, scalar, options);
}

#[rstest]
#[case::overflow("1e999", false)]
#[case::legacy_octal("0_10", true)]
#[case::legacy_hex_separator("0x_FF", true)]
fn quoting_diagnostics_keep_recognized_numeric_text(
    #[case] scalar: &str,
    #[case] legacy_octal_numbers: bool,
) {
    assert_scalar_diagnostics(
        scalar,
        serde_saphyr::options! {
            no_schema: true,
            legacy_octal_numbers: legacy_octal_numbers,
        },
    );
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::expression("2*pi")]
#[case::unit_function("deg(180)")]
#[case::underscored_fraction("1_000.5")]
fn quoting_diagnostics_keep_robotics_text(#[case] scalar: &str) {
    assert_scalar_diagnostics(
        scalar,
        serde_saphyr::options! { no_schema: true, angle_conversions: true },
    );
}

#[cfg(feature = "properties")]
#[rstest]
#[case::overflow("1e999")]
#[case::legacy_octal("0_10")]
#[case::legacy_hex_separator("0x_FF")]
fn interpolated_numeric_text_stays_redacted(#[case] scalar: &str) {
    let options = serde_saphyr::options! {
        no_schema: true,
        legacy_octal_numbers: true,
    }
    .with_properties(std::collections::HashMap::from([(
        "SECRET".to_owned(),
        scalar.to_owned(),
    )]));
    for error in [
        from_str_with_options::<String>("${SECRET}", options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(b"${SECRET}".as_slice(), options.clone())
            .unwrap_err(),
        from_str_with_options::<char>("${SECRET}", options).unwrap_err(),
    ] {
        assert!(!error.to_string().contains(scalar));
        assert_quoting_value(error, "");
    }
}
