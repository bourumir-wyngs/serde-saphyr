#![cfg(feature = "deserialize")]

use std::{collections::BTreeMap, fmt::Debug};

use rstest::rstest;
use serde::{Deserialize, de::DeserializeOwned};
use serde_saphyr::{Error, from_reader, from_str};

fn assert_invalid_scalar<T: DeserializeOwned + Debug>(yaml: &str) {
    for result in [from_str::<T>(yaml), from_reader::<_, T>(yaml.as_bytes())] {
        assert!(
            matches!(
                result.unwrap_err().without_snippet(),
                Error::InvalidScalar { .. }
            ),
            "{yaml} must not deserialize as {}",
            std::any::type_name::<T>(),
        );
    }
}

#[rstest]
#[case::single_quoted("'", "'")]
#[case::double_quoted("\"", "\"")]
#[case::non_specific_tag("! ", "")]
#[case::non_specific_quoted("! '", "'")]
#[case::string_tag("!!str ", "")]
#[case::literal("|-\n  ", "\n")]
#[case::folded(">-\n  ", "\n")]
fn string_scalars_do_not_fill_numeric_or_boolean_targets(
    #[case] prefix: &str,
    #[case] suffix: &str,
) {
    let integer = format!("{prefix}16{suffix}");
    macro_rules! check_integers {
        ($($ty:ty),+ $(,)?) => {$(assert_invalid_scalar::<$ty>(&integer);)+};
    }
    check_integers!(
        i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
    );
    assert_invalid_scalar::<f32>(&integer);
    assert_invalid_scalar::<f64>(&integer);

    let float = format!("{prefix}1.5{suffix}");
    assert_invalid_scalar::<f32>(&float);
    assert_invalid_scalar::<f64>(&float);

    for boolean in ["true", "false", "yes", "off"] {
        assert_invalid_scalar::<bool>(&format!("{prefix}{boolean}{suffix}"));
    }

    for text in ["16", "1.5", "true", "false"] {
        let yaml = format!("{prefix}{text}{suffix}");
        assert_eq!(from_str::<String>(&yaml).unwrap(), text);
        assert_eq!(from_reader::<_, String>(yaml.as_bytes()).unwrap(), text);
        assert_eq!(
            from_str::<serde_json::Value>(&yaml).unwrap(),
            serde_json::Value::String(text.to_owned()),
        );
    }
}

#[rstest]
#[case::plain("", "")]
#[case::single_quoted("'", "'")]
#[case::double_quoted("\"", "\"")]
#[case::literal("|-\n  ", "\n")]
#[case::folded(">-\n  ", "\n")]
fn explicit_numeric_and_boolean_tags_override_scalar_style(
    #[case] prefix: &str,
    #[case] suffix: &str,
) {
    let integer = format!("!!int {prefix}16{suffix}");
    macro_rules! check_integers {
        ($($ty:ty),+ $(,)?) => {$(
            assert_eq!(from_str::<$ty>(&integer).unwrap(), 16);
            assert_eq!(from_reader::<_, $ty>(integer.as_bytes()).unwrap(), 16);
        )+};
    }
    check_integers!(
        i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
    );

    let float = format!("!!float {prefix}1.5{suffix}");
    assert_eq!(from_str::<f32>(&float).unwrap(), 1.5);
    assert_eq!(from_str::<f64>(&float).unwrap(), 1.5);
    assert_eq!(from_reader::<_, f32>(float.as_bytes()).unwrap(), 1.5);
    assert_eq!(from_reader::<_, f64>(float.as_bytes()).unwrap(), 1.5);

    for (scalar, expected) in [("true", true), ("false", false)] {
        let boolean = format!("!!bool {prefix}{scalar}{suffix}");
        assert_eq!(from_str::<bool>(&boolean).unwrap(), expected);
        assert_eq!(
            from_reader::<_, bool>(boolean.as_bytes()).unwrap(),
            expected
        );
    }
}

#[derive(Debug, Deserialize)]
struct Qos {
    depth: u16,
}

#[rstest]
#[case::single_quoted("'16'", 8)]
#[case::double_quoted("\"16\"", 8)]
#[case::non_specific_tag("! 16", 10)]
fn quoted_depth_is_rejected_independently_of_schema_options(
    #[case] scalar: &str,
    #[case] expected_column: u64,
    #[values(false, true)] strict_booleans: bool,
    #[values(false, true)] no_schema: bool,
) {
    let options = serde_saphyr::options! {
        strict_booleans: strict_booleans,
        no_schema: no_schema,
    };
    let yaml = format!("depth: {scalar}\n");
    for result in [
        serde_saphyr::from_str_with_options::<Qos>(&yaml, options.clone()),
        serde_saphyr::from_reader_with_options::<_, Qos>(yaml.as_bytes(), options.clone()),
    ] {
        let error = result.unwrap_err();
        assert!(matches!(
            error.without_snippet(),
            Error::InvalidScalar { ty: "u16", .. }
        ));
        let location = error.location().unwrap();
        assert_eq!((location.line(), location.column()), (1, expected_column));
    }
    assert_eq!(
        serde_saphyr::from_str_with_options::<Qos>("depth: 16\n", options)
            .unwrap()
            .depth,
        16,
    );
}

#[rstest]
#[case::quoted("'16'")]
#[case::non_specific("! 16")]
fn numeric_mapping_keys_preserve_string_scalar_types(#[case] scalar: &str) {
    assert_invalid_scalar::<BTreeMap<u16, String>>(&format!("{scalar}: value\n"));
    assert_invalid_scalar::<BTreeMap<Vec<u16>, String>>(&format!("? [{scalar}]\n: value\n"));
}

#[rstest]
#[case::quoted("'16'")]
#[case::non_specific("! 16")]
fn aliases_preserve_string_scalar_types(#[case] scalar: &str) {
    let yaml = format!("default: &depth {scalar}\ndepth: *depth\n");
    assert!(from_str::<Qos>(&yaml).is_err());
    assert!(from_reader::<_, Qos>(yaml.as_bytes()).is_err());
}

#[rstest]
#[case::single_quoted("'", "'")]
#[case::double_quoted("\"", "\"")]
fn application_tags_do_not_turn_quoted_scalars_into_primitives(
    #[case] prefix: &str,
    #[case] suffix: &str,
) {
    assert_invalid_scalar::<u16>(&format!("!custom {prefix}16{suffix}"));
    assert_invalid_scalar::<f64>(&format!("!custom {prefix}1.5{suffix}"));
    assert_invalid_scalar::<bool>(&format!("!custom {prefix}true{suffix}"));
}

#[derive(Debug, Deserialize, PartialEq)]
enum Primitive {
    Integer(u16),
    Float(f64),
    Boolean(bool),
}

#[rstest]
#[case::integer("Integer", "16", Primitive::Integer(16))]
#[case::float("Float", "1.5", Primitive::Float(1.5))]
#[case::boolean("Boolean", "true", Primitive::Boolean(true))]
fn enum_variant_tags_preserve_payload_style(
    #[case] variant: &str,
    #[case] scalar: &str,
    #[case] expected: Primitive,
) {
    for quote in ["'", "\""] {
        assert_invalid_scalar::<Primitive>(&format!("!{variant} {quote}{scalar}{quote}"));
    }
    let plain = format!("!{variant} {scalar}");
    assert_eq!(from_str::<Primitive>(&plain).unwrap(), expected);
    assert_eq!(
        from_reader::<_, Primitive>(plain.as_bytes()).unwrap(),
        expected
    );
}

#[cfg(feature = "properties")]
#[rstest]
fn plain_compose_interpolations_still_fill_numeric_and_boolean_targets(
    #[values(
        serde_saphyr::PropertySyntax::DockerCompose,
        serde_saphyr::PropertySyntax::DockerComposeWithNulls
    )]
    syntax: serde_saphyr::PropertySyntax,
) {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Config {
        integer: u16,
        float: f64,
        boolean: bool,
    }

    let options = serde_saphyr::options! { property_syntax: syntax }.with_properties(
        [("INTEGER", "16"), ("FLOAT", "1.5"), ("BOOLEAN", "true")]
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
    );
    let yaml = "integer: ${INTEGER}\nfloat: ${FLOAT}\nboolean: ${BOOLEAN}\n";
    let expected = Config {
        integer: 16,
        float: 1.5,
        boolean: true,
    };
    assert_eq!(
        serde_saphyr::from_str_with_options::<Config>(yaml, options.clone()).unwrap(),
        expected,
    );
    assert_eq!(
        serde_saphyr::from_reader_with_options::<_, Config>(yaml.as_bytes(), options.clone())
            .unwrap(),
        expected,
    );

    macro_rules! check_rejected {
        ($ty:ty, $name:literal) => {
            for (prefix, suffix) in [("'", "'"), ("\"", "\""), ("! ", "")] {
                let yaml = format!("{prefix}${{{}}}{suffix}", $name);
                for result in [
                    serde_saphyr::from_str_with_options::<$ty>(&yaml, options.clone()),
                    serde_saphyr::from_reader_with_options::<_, $ty>(
                        yaml.as_bytes(),
                        options.clone(),
                    ),
                ] {
                    assert!(matches!(
                        result.unwrap_err().without_snippet(),
                        Error::InvalidScalar { .. }
                    ));
                }
            }
        };
    }
    check_rejected!(u16, "INTEGER");
    check_rejected!(f64, "FLOAT");
    check_rejected!(bool, "BOOLEAN");
}
