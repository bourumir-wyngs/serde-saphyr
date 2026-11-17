#![cfg(feature = "deserialize")]

use serde_json::Value;
use serde_saphyr::NonFiniteFloatPolicy;

const MIXED_CASE_NON_FINITE_STRINGS: [&str; 8] = [
    ".iNf", ".InF", ".inF", "-.InF", "+.iNF", ".nAn", ".Nan", ".naN",
];

#[test]
fn nonstandard_non_finite_spellings_remain_strings() -> Result<(), serde_saphyr::Error> {
    for text in MIXED_CASE_NON_FINITE_STRINGS {
        let yaml = format!("--- {text}\n...\n");

        assert_eq!(
            serde_saphyr::from_str::<Value>(&yaml)?,
            Value::String(text.to_owned())
        );

        let options = serde_saphyr::options! {
            non_finite_float_policy: NonFiniteFloatPolicy::AsString,
        };
        assert_eq!(
            serde_saphyr::from_str_with_options::<Value>(&yaml, options)?,
            Value::String(text.to_owned())
        );

        let options = serde_saphyr::options! { no_schema: true };
        assert_eq!(
            serde_saphyr::from_str_with_options::<String>(&yaml, options)?,
            text
        );

        assert!(serde_saphyr::from_str::<f64>(&yaml).is_err(), "{text:?}");
        assert!(serde_saphyr::from_str::<f32>(&yaml).is_err(), "{text:?}");
    }

    Ok(())
}

#[test]
fn standard_non_finite_spellings_remain_floats() -> Result<(), serde_saphyr::Error> {
    for text in [".inf", ".Inf", ".INF", "+.inf", "+.Inf", "+.INF"] {
        assert_eq!(
            serde_saphyr::from_str::<f64>(text)?,
            f64::INFINITY,
            "{text:?}"
        );
    }
    for text in ["-.inf", "-.Inf", "-.INF"] {
        assert_eq!(
            serde_saphyr::from_str::<f64>(text)?,
            f64::NEG_INFINITY,
            "{text:?}"
        );
    }
    for text in [".nan", ".NaN", ".NAN", "+.nan", "-.NaN", "-.NAN"] {
        assert!(serde_saphyr::from_str::<f64>(text)?.is_nan(), "{text:?}");
        assert!(serde_saphyr::from_str::<f32>(text)?.is_nan(), "{text:?}");
    }

    Ok(())
}
