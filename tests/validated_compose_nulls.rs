#![cfg(all(feature = "properties", any(feature = "garde", feature = "validator")))]

use rstest::rstest;
use serde::{Deserialize, de::DeserializeOwned};
use serde_saphyr::{Error, Options, PropertySyntax, ValidationSource, budget::BudgetBreach};
use std::collections::HashMap;

const SECRET: &str = "secret-must-never-appear-in-validation-errors";

fn options() -> Options {
    serde_saphyr::options! { property_syntax: PropertySyntax::DockerComposeWithNulls }
        .with_properties(HashMap::from([
            ("SET".into(), "kept".into()),
            ("EMPTY".into(), String::new()),
            ("NUL".into(), "null".into()),
            ("SECRET".into(), SECRET.into()),
        ]))
}

fn work_options(max_work: usize) -> Options {
    let mut options = options();
    options.budget = serde_saphyr::budget! {
        max_total_property_interpolation_work: max_work,
    };
    options
}

#[derive(Debug, Deserialize, PartialEq)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
struct Scalar(#[cfg_attr(feature = "garde", garde(skip))] String);

#[derive(Debug, Deserialize, PartialEq)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
enum Tagged {
    Maybe(#[cfg_attr(feature = "garde", garde(skip))] Option<String>),
}

#[cfg(feature = "validator")]
impl validator::Validate for Scalar {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

#[cfg(feature = "validator")]
impl validator::Validate for Tagged {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
struct Checked {
    #[cfg_attr(feature = "garde", garde(custom(check_garde)))]
    #[cfg_attr(feature = "validator", validate(custom(function = "check_validator")))]
    value: String,
}

#[cfg(feature = "garde")]
fn check_garde(value: &str, _context: &()) -> garde::Result {
    if value == "ok" {
        Ok(())
    } else {
        Err(garde::Error::new(format!("invalid value: {value}")))
    }
}

#[cfg(feature = "validator")]
fn check_validator(value: &str) -> Result<(), validator::ValidationError> {
    if value == "ok" {
        Ok(())
    } else {
        let mut error = validator::ValidationError::new("invalid_value");
        error.message = Some(format!("invalid value: {value}").into());
        Err(error)
    }
}

fn assert_work_budget(error: &Error) {
    assert!(matches!(
        error.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::PropertyInterpolationWork { .. },
            ..
        }
    ));
}

// Run the same assertions with each validation backend, including configurations
// where only one optional validation dependency is enabled.
macro_rules! validation_tests {
    ($module:ident, $bound:path, $single:ident, $multiple:ident, $slice:ident, $read:ident, $source:expr) => {
        mod $module {
            use super::*;

            fn streams<T: DeserializeOwned + $bound>(
                yaml: &str,
                options: Options,
            ) -> [Result<Vec<T>, Error>; 3] {
                let mut reader = yaml.as_bytes();
                [
                    serde_saphyr::$multiple(yaml, options.clone()),
                    serde_saphyr::$slice(yaml.as_bytes(), options.clone()),
                    serde_saphyr::$read(&mut reader, options).collect(),
                ]
            }

            #[rstest]
            #[case::missing("$MISSING", None)]
            #[case::marker("$NUL", None)]
            #[case::default("${MISSING:-null}", None)]
            #[case::empty("$EMPTY", Some(""))]
            #[case::string_tag("!!str $MISSING", Some(""))]
            #[case::non_specific_tag("! $MISSING", Some(""))]
            fn scalar_streams_match_ordinary(
                #[case] first: &str,
                #[case] expected_first: Option<&str>,
            ) {
                let yaml = format!("{first}\n---\nkept\n");
                let mut expected = Vec::new();
                if let Some(value) = expected_first {
                    expected.push(Scalar(value.into()));
                }
                expected.push(Scalar("kept".into()));
                let ordinary =
                    serde_saphyr::from_multiple_with_options::<Scalar>(&yaml, options()).unwrap();
                assert_eq!(ordinary, expected);
                for result in streams::<Scalar>(&yaml, options()) {
                    assert_eq!(result.unwrap(), expected);
                }
            }

            #[rstest]
            #[case::literal("!Maybe null")]
            #[case::missing("!Maybe $MISSING")]
            fn tagged_null_payloads_reach_validation(#[case] first: &str) {
                let yaml = format!("{first}\n---\n!Maybe kept\n");
                let expected = vec![Tagged::Maybe(None), Tagged::Maybe(Some("kept".into()))];
                let ordinary =
                    serde_saphyr::from_multiple_with_options::<Tagged>(&yaml, options()).unwrap();
                assert_eq!(ordinary, expected);
                for result in streams::<Tagged>(&yaml, options()) {
                    assert_eq!(result.unwrap(), expected);
                }
            }

            #[rstest]
            #[case::property("$SET")]
            #[case::literal("kept")]
            #[case::escape("$$SET")]
            fn exact_work_budget_matches_ordinary(#[case] yaml: &str) {
                let work = (1..=128)
                    .find(|&limit| {
                        serde_saphyr::from_str_with_options::<Scalar>(yaml, work_options(limit))
                            .is_ok()
                    })
                    .unwrap();
                let expected =
                    serde_saphyr::from_str_with_options::<Scalar>(yaml, work_options(work))
                        .unwrap();
                assert_eq!(
                    serde_saphyr::$single::<Scalar>(yaml, work_options(work)).unwrap(),
                    expected
                );
                for result in streams::<Scalar>(yaml, work_options(work)) {
                    assert_eq!(result.unwrap(), [Scalar(expected.0.clone())]);
                }
                for result in streams::<Scalar>(yaml, work_options(work - 1)) {
                    assert_work_budget(&result.unwrap_err());
                }
            }

            #[test]
            fn validation_aggregates_with_locations_and_redaction() {
                let yaml = "$MISSING\n---\nvalue: $SECRET\n---\nvalue: ''\n---\nvalue: ok\n";
                for result in [
                    serde_saphyr::$multiple::<Checked>(yaml, options()),
                    serde_saphyr::$slice::<Checked>(yaml.as_bytes(), options()),
                ] {
                    let error = result.unwrap_err();
                    let Error::ValidationErrors { source, errors } = error.without_snippet() else {
                        panic!("expected aggregated validation errors: {error:?}");
                    };
                    assert_eq!(*source, $source);
                    assert_eq!(errors.len(), 2);
                    assert_eq!(errors[0].location().unwrap().line(), 3);
                    assert_eq!(errors[1].location().unwrap().line(), 5);
                    assert!(!error.to_string().contains(SECRET), "{error}");
                }

                let mut reader = yaml.as_bytes();
                let mut values = serde_saphyr::$read::<_, Checked>(&mut reader, options());
                for line in [3, 5] {
                    let error = values.next().unwrap().unwrap_err();
                    assert!(matches!(
                        error.without_snippet(),
                        Error::ValidationError { .. }
                    ));
                    assert_eq!(error.location().unwrap().line(), line);
                    assert!(!error.to_string().contains(SECRET), "{error}");
                }
                assert_eq!(values.next().unwrap().unwrap().value, "ok");
                assert!(values.next().is_none());
            }

            #[test]
            fn aliased_interpolation_budget_error_ends_reader() {
                let yaml = "&a $SET: ignored\nvalue: *a\n---\nvalue: ok\n";
                let mut options = options();
                // Allow the ignored field and alias redaction peek, then fail inside the alias.
                options.budget =
                    serde_saphyr::budget! { max_total_property_interpolation_work: 27 };
                let mut reader = yaml.as_bytes();
                let mut values = serde_saphyr::$read::<_, Checked>(&mut reader, options);
                let error = values.next().unwrap().unwrap_err();
                let Error::AliasError { error, .. } = error.without_snippet() else {
                    panic!("expected an alias error: {error:?}");
                };
                assert!(matches!(
                    error.without_snippet(),
                    Error::Budget {
                        breach: BudgetBreach::PropertyInterpolationWork { .. },
                        ..
                    }
                ));
                assert!(values.next().is_none());
            }

            #[test]
            fn required_error_recovers_but_work_budget_is_terminal() {
                let yaml = "${MISSING:?required}\n---\nkept\n";
                let mut reader = yaml.as_bytes();
                let mut values = serde_saphyr::$read::<_, Scalar>(&mut reader, options());
                let error = values.next().unwrap().unwrap_err();
                assert!(matches!(
                    error.without_snippet(),
                    Error::PropertyRequiredButUnset { .. }
                ));
                assert_eq!(values.next().unwrap().unwrap(), Scalar("kept".into()));
                assert!(values.next().is_none());

                let mut reader = "$SET\n---\nkept\n".as_bytes();
                let mut values = serde_saphyr::$read::<_, Scalar>(&mut reader, work_options(0));
                assert_work_budget(&values.next().unwrap().unwrap_err());
                assert!(values.next().is_none());
            }
        }
    };
}

#[cfg(feature = "garde")]
validation_tests!(
    garde_backend,
    garde::Validate<Context = ()>,
    from_str_with_options_valid,
    from_multiple_with_options_valid,
    from_slice_multiple_with_options_valid,
    read_with_options_valid,
    ValidationSource::Garde
);

#[cfg(feature = "validator")]
validation_tests!(
    validator_backend,
    validator::Validate,
    from_str_with_options_validate,
    from_multiple_with_options_validate,
    from_slice_multiple_with_options_validate,
    read_with_options_validate,
    ValidationSource::Validator
);
