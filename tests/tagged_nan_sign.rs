#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{
    NonFiniteFloatPolicy, from_reader_with_options, from_str, from_str_with_options,
};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BufferedFloat {
    Float(f64),
}

#[rstest]
#[case("+.nan", false)]
#[case("+.NaN", false)]
#[case("+.NAN", false)]
#[case("-.nan", true)]
#[case("-.NaN", true)]
#[case("-.NAN", true)]
fn explicit_float_preserves_nan_sign(#[case] scalar: &str, #[case] negative: bool) {
    for angle_conversions in [false, true] {
        let options = serde_saphyr::options! {
            angle_conversions: angle_conversions,
            non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
        };
        for yaml in [
            format!("!!float {scalar}"),
            format!("!!float '{scalar}'"),
            format!("!!float \"  {scalar}  \""),
            format!("!!float |\n  {scalar}\n"),
            format!("!!float >\n  {scalar}\n"),
        ] {
            for value in [
                from_str_with_options::<f32>(&yaml, options.clone()).unwrap(),
                from_reader_with_options::<_, f32>(yaml.as_bytes(), options.clone()).unwrap(),
            ] {
                assert!(value.is_nan(), "{yaml}");
                assert_eq!(value.is_sign_negative(), negative, "{yaml}");
            }
            let BufferedFloat::Float(buffered) =
                from_str_with_options(&yaml, options.clone()).unwrap();
            for value in [
                from_str_with_options::<f64>(&yaml, options.clone()).unwrap(),
                from_reader_with_options::<_, f64>(yaml.as_bytes(), options.clone()).unwrap(),
                buffered,
            ] {
                assert!(value.is_nan(), "{yaml}");
                assert_eq!(value.is_sign_negative(), negative, "{yaml}");
            }
        }
    }
}

#[rstest]
fn untagged_typed_nan_keeps_its_existing_sign(
    #[values("+.nan", "+.NaN", "+.NAN", "-.nan", "-.NaN", "-.NAN")] scalar: &str,
) {
    let canonical32 = from_str::<f32>(".nan").unwrap();
    let canonical64 = from_str::<f64>(".nan").unwrap();
    let value32 = from_str::<f32>(scalar).unwrap();
    let value64 = from_str::<f64>(scalar).unwrap();
    assert!(value32.is_nan());
    assert!(value64.is_nan());
    assert_eq!(value32.is_sign_negative(), canonical32.is_sign_negative());
    assert_eq!(value64.is_sign_negative(), canonical64.is_sign_negative());
}
