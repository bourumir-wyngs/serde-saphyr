#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{Error, from_reader_with_options, from_str_with_options, options};
use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct BorrowedText<'a> {
    #[serde(borrow)]
    value: Cow<'a, str>,
}

#[rstest]
fn no_schema_accepts_signed_nan_strings(
    #[values("+.nan", "+.NaN", "+.NAN", "-.nan", "-.NaN", "-.NAN")] scalar: &str,
) {
    let options = options! { no_schema: true };
    let owned: String = from_str_with_options(scalar, options.clone()).unwrap();
    let borrowed: &str = from_str_with_options(scalar, options.clone()).unwrap();
    let cow: Cow<'_, str> = from_str_with_options(scalar, options.clone()).unwrap();
    let reader: String = from_reader_with_options(scalar.as_bytes(), options.clone()).unwrap();
    assert_eq!(owned, scalar);
    assert_eq!(borrowed, scalar);
    assert_eq!(cow, scalar);
    assert_eq!(reader, scalar);

    let yaml = format!("value: {scalar}\n");
    let value: BorrowedText<'_> = from_str_with_options(&yaml, options).unwrap();
    assert!(matches!(value.value, Cow::Borrowed(text) if text == scalar));
}

#[rstest]
fn no_schema_accepts_signed_nan_mapping_keys_and_values(
    #[values("+.nan", "+.NaN", "+.NAN", "-.nan", "-.NaN", "-.NAN")] scalar: &str,
) {
    let options = options! { no_schema: true };
    let yaml = format!("{scalar}: {scalar}\n");
    let owned: BTreeMap<String, String> = from_str_with_options(&yaml, options.clone()).unwrap();
    let borrowed: BTreeMap<&str, &str> = from_str_with_options(&yaml, options).unwrap();
    assert_eq!(owned[scalar], scalar);
    assert_eq!(borrowed[scalar], scalar);
}

#[rstest]
fn no_schema_still_requires_quotes_for_non_finite_numbers(
    #[values(
        ".nan", ".NaN", ".NAN", ".inf", ".Inf", ".INF", "+.inf", "+.Inf", "+.INF", "-.inf",
        "-.Inf", "-.INF"
    )]
    scalar: &str,
) {
    let options = options! { no_schema: true };
    for error in [
        from_str_with_options::<String>(scalar, options.clone()).unwrap_err(),
        from_str_with_options::<&str>(scalar, options.clone()).unwrap_err(),
        from_str_with_options::<Cow<'_, str>>(scalar, options).unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::QuotingRequired { value, .. } if value == scalar
        ));
    }
}

#[rstest]
fn no_schema_preserves_typed_and_tagged_signed_nan(
    #[values("+.nan", "+.NaN", "+.NAN", "-.nan", "-.NaN", "-.NAN")] scalar: &str,
) {
    let options = options! { no_schema: true };
    for yaml in [scalar.to_owned(), format!("!!float {scalar}")] {
        assert!(
            from_str_with_options::<f32>(&yaml, options.clone())
                .unwrap()
                .is_nan()
        );
        assert!(
            from_str_with_options::<f64>(&yaml, options.clone())
                .unwrap()
                .is_nan()
        );
    }
    let yaml = format!("!!float {scalar}");
    for error in [
        from_str_with_options::<String>(&yaml, options.clone()).unwrap_err(),
        from_str_with_options::<&str>(&yaml, options).unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::TaggedScalarCannotDeserializeIntoString { .. }
        ));
    }
}
