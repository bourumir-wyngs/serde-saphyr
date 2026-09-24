use serde_saphyr::scalar::{ScalarError, ScalarKind, ScalarStyle, Schema, resolve};

const STRING_TAG: &str = "tag:yaml.org,2002:str";
const NULL_TAG: &str = "tag:yaml.org,2002:null";
const BOOL_TAG: &str = "tag:yaml.org,2002:bool";
const INT_TAG: &str = "tag:yaml.org,2002:int";
const FLOAT_TAG: &str = "tag:yaml.org,2002:float";
const TIMESTAMP_TAG: &str = "tag:yaml.org,2002:timestamp";

fn assert_kind(text: &str, schema: Schema, expected: ScalarKind) {
    let scalar = resolve(text, ScalarStyle::Plain, None, schema)
        .unwrap_or_else(|error| panic!("{text:?} under {schema:?}: {error}"));
    assert_eq!(scalar.kind(), expected, "{text:?} under {schema:?}");
    assert_eq!(scalar.text(), text);
}

#[test]
fn resolution_borrows_the_original_text() {
    let owned = String::from("00042");
    let scalar = resolve(&owned, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(scalar.kind(), ScalarKind::Integer);
    assert_eq!(scalar.text(), "00042");
    assert_eq!(scalar.text().as_ptr(), owned.as_ptr());
    assert_eq!(scalar.to_i128(), Ok(42));
}

#[cfg(feature = "deserialize")]
#[test]
fn parser_events_supply_decoded_text_style_and_expanded_tag() {
    use serde_saphyr::granit_parser::{Event, Parser};

    let yaml = "%TAG !number! tag:yaml.org,2002:\n---\n- !number!int \"\\u0034\\u0032\"\n- \"42\"\n- a.infra\n";
    let mut kinds = Vec::new();
    for item in Parser::new_from_str(yaml) {
        if let Event::Scalar(text, style, _, tag) = item.unwrap().0 {
            let expanded_tag = tag.map(|tag| tag.to_string());
            let scalar =
                resolve(&text, style.into(), expanded_tag.as_deref(), Schema::Yaml12).unwrap();
            if kinds.is_empty() {
                assert_eq!(expanded_tag.as_deref(), Some(INT_TAG));
                assert_eq!(scalar.text(), "42");
                assert_eq!(scalar.to_i128(), Ok(42));
            }
            kinds.push(scalar.kind());
        }
    }
    assert_eq!(
        kinds,
        [ScalarKind::Integer, ScalarKind::String, ScalarKind::String]
    );
}

#[test]
fn float_like_substrings_remain_strings() {
    for schema in [Schema::Strings, Schema::Yaml12, Schema::Yaml11] {
        for text in [
            "a.infra",
            "a.nanotube",
            ".infinite",
            ".nanotube",
            "prefix.inf",
            ".inf.suffix",
            "prefix.nan",
            ".nan.suffix",
            "infinity",
            "inf",
            "nan",
            "NaN",
        ] {
            assert_kind(text, schema, ScalarKind::String);
        }
    }
}

#[test]
fn strings_keeps_every_implicit_scalar_as_text() {
    for text in ["", "null", "~", "true", "123", "1.25", ".inf", "2001-12-15"] {
        assert_kind(text, Schema::Strings, ScalarKind::String);
    }
}

#[test]
fn non_plain_styles_disable_implicit_typing() {
    for schema in [
        Schema::Strings,
        Schema::Json,
        Schema::Yaml12,
        Schema::Yaml11,
    ] {
        for style in [
            ScalarStyle::SingleQuoted,
            ScalarStyle::DoubleQuoted,
            ScalarStyle::Literal,
            ScalarStyle::Folded,
        ] {
            for text in ["", "true", "null", "42", ".inf", "2001-12-15", "word"] {
                let scalar = resolve(text, style, None, schema).unwrap();
                assert_eq!(
                    scalar.kind(),
                    ScalarKind::String,
                    "{schema:?}, {style:?}, {text:?}"
                );
                assert_eq!(scalar.text(), text);
            }
        }
    }
}

#[test]
fn nonspecific_tags_preserve_style_and_schema_rules() {
    for schema in [
        Schema::Strings,
        Schema::Json,
        Schema::Yaml12,
        Schema::Yaml11,
    ] {
        for style in [ScalarStyle::Plain, ScalarStyle::DoubleQuoted] {
            let scalar = resolve("42", style, Some("!"), schema).unwrap();
            assert_eq!(scalar.kind(), ScalarKind::String);
            let implicit = resolve("42", style, None, schema).unwrap();
            let question = resolve("42", style, Some("?"), schema).unwrap();
            assert_eq!(question.kind(), implicit.kind());
        }
    }
}

#[test]
fn explicit_tags_override_style() {
    for style in [
        ScalarStyle::Plain,
        ScalarStyle::SingleQuoted,
        ScalarStyle::DoubleQuoted,
        ScalarStyle::Literal,
        ScalarStyle::Folded,
    ] {
        for (text, tag, kind) in [
            ("true", STRING_TAG, ScalarKind::String),
            ("null", NULL_TAG, ScalarKind::Null),
            ("true", BOOL_TAG, ScalarKind::Boolean),
            ("42", INT_TAG, ScalarKind::Integer),
            ("42", FLOAT_TAG, ScalarKind::Float),
        ] {
            assert_eq!(
                resolve(text, style, Some(tag), Schema::Yaml12)
                    .unwrap()
                    .kind(),
                kind
            );
        }
    }
}

#[test]
fn only_expanded_supported_tags_are_accepted() {
    for tag in [
        "",
        "!!int",
        "!int",
        "!custom",
        "!<tag:yaml.org,2002:int>",
        "tag:example.com,2026:int",
        "tag:yaml.org,2002:binary",
        "tag:yaml.org,2002:seq",
        "tag:yaml.org,2002:map",
        "tag:yaml.org,2002:Int",
    ] {
        assert_eq!(
            resolve("42", ScalarStyle::Plain, Some(tag), Schema::Yaml12).unwrap_err(),
            ScalarError::UnsupportedTag,
            "{tag:?}"
        );
    }
}

#[test]
fn explicit_tags_are_limited_by_the_schema() {
    for tag in [NULL_TAG, BOOL_TAG, INT_TAG, FLOAT_TAG, TIMESTAMP_TAG] {
        assert_eq!(
            resolve("42", ScalarStyle::Plain, Some(tag), Schema::Strings).unwrap_err(),
            ScalarError::UnsupportedTag
        );
    }
    for schema in [Schema::Json, Schema::Yaml12] {
        assert_eq!(
            resolve(
                "2001-12-15",
                ScalarStyle::Plain,
                Some(TIMESTAMP_TAG),
                schema
            )
            .unwrap_err(),
            ScalarError::UnsupportedTag
        );
    }
    assert_eq!(
        resolve("42", ScalarStyle::Plain, Some(STRING_TAG), Schema::Strings)
            .unwrap()
            .kind(),
        ScalarKind::String
    );
}

#[test]
fn recognized_tags_validate_instead_of_falling_back_to_strings() {
    for (text, tag, kind, schema) in [
        ("yes", BOOL_TAG, ScalarKind::Boolean, Schema::Yaml12),
        ("Null", NULL_TAG, ScalarKind::Null, Schema::Json),
        ("0b10", INT_TAG, ScalarKind::Integer, Schema::Yaml12),
        (".INF", FLOAT_TAG, ScalarKind::Float, Schema::Json),
        ("1e2", FLOAT_TAG, ScalarKind::Float, Schema::Yaml11),
        ("a.infra", FLOAT_TAG, ScalarKind::Float, Schema::Yaml12),
        ("a.nanotube", FLOAT_TAG, ScalarKind::Float, Schema::Yaml11),
        (
            "yesterday",
            TIMESTAMP_TAG,
            ScalarKind::Timestamp,
            Schema::Yaml11,
        ),
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::DoubleQuoted, Some(tag), schema).unwrap_err(),
            ScalarError::InvalidValue { kind },
            "{text:?}, {tag:?}, {schema:?}"
        );
    }
}

#[test]
fn scalar_text_is_not_trimmed_or_unescaped() {
    for text in [
        " true", "true ", "42\n", "\t42", " .inf", "null\r", "\\u0031",
    ] {
        assert_kind(text, Schema::Yaml12, ScalarKind::String);
    }
    for (text, tag, kind) in [
        (" true", BOOL_TAG, ScalarKind::Boolean),
        ("42\n", INT_TAG, ScalarKind::Integer),
        (".inf ", FLOAT_TAG, ScalarKind::Float),
        ("null\r", NULL_TAG, ScalarKind::Null),
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::DoubleQuoted, Some(tag), Schema::Yaml12).unwrap_err(),
            ScalarError::InvalidValue { kind }
        );
    }
}

#[test]
fn explicit_float_tags_accept_canonical_forms_outside_implicit_patterns() {
    for (text, expected) in [(".inf", f64::INFINITY), ("-.inf", f64::NEG_INFINITY)] {
        let scalar = resolve(
            text,
            ScalarStyle::DoubleQuoted,
            Some(FLOAT_TAG),
            Schema::Json,
        )
        .unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Float);
        assert_eq!(scalar.to_f64(), Ok(expected));
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Json).unwrap_err(),
            ScalarError::UnresolvedPlainScalar
        );
    }
    let scalar = resolve(".nan", ScalarStyle::Plain, Some(FLOAT_TAG), Schema::Json).unwrap();
    assert!(scalar.to_f64().unwrap().is_nan());
    for (text, expected) in [("0", 0.0), ("42", 42.0), ("-42", -42.0), ("+1_000", 1000.0)] {
        let scalar = resolve(
            text,
            ScalarStyle::DoubleQuoted,
            Some(FLOAT_TAG),
            Schema::Yaml11,
        )
        .unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Float);
        assert_eq!(scalar.to_f64(), Ok(expected));
    }
}

#[test]
fn json_schema_uses_yaml_json_resolution_patterns() {
    for (text, kind) in [
        ("null", ScalarKind::Null),
        ("true", ScalarKind::Boolean),
        ("false", ScalarKind::Boolean),
        ("0", ScalarKind::Integer),
        ("-0", ScalarKind::Integer),
        ("123", ScalarKind::Integer),
        ("-123", ScalarKind::Integer),
        ("0.5", ScalarKind::Float),
        ("1.", ScalarKind::Float),
        ("-0.", ScalarKind::Float),
        ("1.e2", ScalarKind::Float),
        ("12e03", ScalarKind::Float),
        ("-2E+05", ScalarKind::Float),
    ] {
        assert_kind(text, Schema::Json, kind);
    }
    for text in [
        "", "word", "a.infra", "True", "NULL", "~", "+1", "01", "-01", ".5", "01.5", "0x10",
        "0o10", ".inf", ".nan", "1_000", "1e", "1e+", "1.0tail",
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Json).unwrap_err(),
            ScalarError::UnresolvedPlainScalar,
            "{text:?}"
        );
    }
}

#[test]
fn yaml12_schema_recognizes_null_and_boolean_spellings() {
    for text in ["", "~", "null", "Null", "NULL"] {
        assert_kind(text, Schema::Yaml12, ScalarKind::Null);
    }
    for text in ["true", "True", "TRUE"] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
        assert_eq!(scalar.to_bool(), Ok(true));
    }
    for text in ["false", "False", "FALSE"] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
        assert_eq!(scalar.to_bool(), Ok(false));
    }
    for text in ["nUlL", "tRuE", "fAlSe", "yes", "no", "on", "off", "y", "n"] {
        assert_kind(text, Schema::Yaml12, ScalarKind::String);
    }
}

#[test]
fn yaml12_integer_syntax_and_radix_conversion() {
    for (text, expected) in [
        ("0", 0),
        ("+0", 0),
        ("-0", 0),
        ("0010", 10),
        ("+0010", 10),
        ("-0010", -10),
        ("0o52", 42),
        ("0x2a", 42),
        ("0x2A", 42),
    ] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Integer, "{text:?}");
        assert_eq!(scalar.to_i128(), Ok(expected), "{text:?}");
    }
    for text in [
        "0b10", "0B10", "0O52", "0X2A", "-0x2a", "+0o52", "1_000", "0x2_A", "0x", "0o", "0o8",
        "0xG", "1:02", "12tail", "１２", "+", "--1",
    ] {
        assert_kind(text, Schema::Yaml12, ScalarKind::String);
    }
}

#[test]
fn yaml12_float_syntax_and_conversion() {
    for (text, expected) in [
        ("0.", 0.0),
        (".5", 0.5),
        ("-.5", -0.5),
        ("+12e03", 12000.0),
        ("-2E+05", -200000.0),
        ("01.50", 1.5),
        ("1.e2", 100.0),
    ] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Float, "{text:?}");
        assert_eq!(scalar.to_f64(), Ok(expected), "{text:?}");
        assert_eq!(scalar.to_f32(), Ok(expected as f32), "{text:?}");
    }
    for text in [
        ".", "+.", "1e", "1e+", "1.2.3", "1_0.0", "1.0_0", "0x1p2", "+.nan", "-.nan", ".nAn",
        ".iNF",
    ] {
        assert_kind(text, Schema::Yaml12, ScalarKind::String);
    }
}

#[test]
fn yaml11_boolean_spellings_are_exact() {
    for text in [
        "y", "Y", "yes", "Yes", "YES", "true", "True", "TRUE", "on", "On", "ON",
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Yaml11)
                .unwrap()
                .to_bool(),
            Ok(true),
            "{text:?}"
        );
    }
    for text in [
        "n", "N", "no", "No", "NO", "false", "False", "FALSE", "off", "Off", "OFF",
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Yaml11)
                .unwrap()
                .to_bool(),
            Ok(false),
            "{text:?}"
        );
    }
    for text in ["yEs", "nO", "oN", "ofF", "tRuE", "fAlSe"] {
        assert_kind(text, Schema::Yaml11, ScalarKind::String);
    }
}

#[test]
fn yaml11_integer_bases_and_separators() {
    for (text, expected) in [
        ("0b10_10", 10),
        ("-0b10", -2),
        ("+0x2A", 42),
        ("0x_2_A_", 42),
        ("052", 42),
        ("-052", -42),
        ("1_000", 1000),
        ("1__0_", 10),
        ("1:02", 62),
        ("-1:02:03", -3723),
        ("190:20:30", 685230),
    ] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml11).unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Integer, "{text:?}");
        assert_eq!(scalar.to_i128(), Ok(expected), "{text:?}");
    }
    for text in [
        "08", "09", "0o52", "0X2A", "0B10", "0x__", "0b__", "_1", "0:10", "1:60", "1:001", "1:0_1",
    ] {
        assert_kind(text, Schema::Yaml11, ScalarKind::String);
    }
}

#[test]
fn yaml11_floats_require_a_dot_and_signed_exponent() {
    for (text, expected) in [
        (".5", 0.5),
        ("1.", 1.0),
        ("1.5e+2", 150.0),
        ("1_000.2_5", 1000.25),
        ("1:02.5", 62.5),
        ("-1:02:03.25", -3723.25),
        ("190:20:30.15", 685230.15),
    ] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml11).unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Float, "{text:?}");
        assert_eq!(scalar.to_f64(), Ok(expected), "{text:?}");
    }
    for text in [
        "1e+2", "1.5e2", ".", "..", "1.2.3", "1:60.0", "1:2e+1", "-.nan",
    ] {
        assert_kind(text, Schema::Yaml11, ScalarKind::String);
    }
}

#[test]
fn timestamp_classification_is_yaml11_only_and_lexical() {
    for text in [
        "2001-12-15",
        "2001-12-15T02:59:43.1Z",
        "2001-12-14t21:59:43.10-05:00",
        "2001-12-14 21:59:43.10 -5",
        "2001-12-15 2:59:43.10",
        "2001-1-2T3:04:05Z",
        "2001-12-15\t02:59:43 Z",
        "2001-12-15T02:59:43.Z",
        "2001-99-99",
    ] {
        assert_kind(text, Schema::Yaml11, ScalarKind::Timestamp);
        assert_kind(text, Schema::Yaml12, ScalarKind::String);
        let explicit = resolve(
            text,
            ScalarStyle::DoubleQuoted,
            Some(TIMESTAMP_TAG),
            Schema::Yaml11,
        )
        .unwrap();
        assert_eq!(explicit.kind(), ScalarKind::Timestamp);
    }
    for text in [
        "2001-1-2",
        "01-12-15",
        "2001-12-15tail",
        "2001-12-15T02:59",
        "2001-12-15T02:59:43z",
        "2001-12-15T02:59:43+0500",
        "2001-12-15T02:59:43Ztail",
    ] {
        assert_kind(text, Schema::Yaml11, ScalarKind::String);
    }
}

#[test]
fn integer_classification_is_independent_of_machine_range() {
    let huge = "9".repeat(4096);
    for schema in [Schema::Json, Schema::Yaml12, Schema::Yaml11] {
        let scalar = resolve(&huge, ScalarStyle::Plain, None, schema).unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Integer);
        assert_eq!(scalar.text().as_ptr(), huge.as_ptr());
        assert_eq!(scalar.to_i128(), Err(ScalarError::OutOfRange));
        assert_eq!(scalar.to_u128(), Err(ScalarError::OutOfRange));
    }
    let leading_zeros = format!("{}42", "0".repeat(4096));
    let scalar = resolve(&leading_zeros, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(scalar.to_i128(), Ok(42));
}

#[test]
fn signed_and_unsigned_integer_boundaries_are_checked() {
    for (text, expected) in [
        ("-170141183460469231731687303715884105728", i128::MIN),
        ("170141183460469231731687303715884105727", i128::MAX),
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Yaml12)
                .unwrap()
                .to_i128(),
            Ok(expected)
        );
    }
    for text in [
        "-170141183460469231731687303715884105729",
        "170141183460469231731687303715884105728",
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Yaml12)
                .unwrap()
                .to_i128(),
            Err(ScalarError::OutOfRange)
        );
    }
    let scalar = resolve(
        "340282366920938463463374607431768211455",
        ScalarStyle::Plain,
        None,
        Schema::Yaml12,
    )
    .unwrap();
    assert_eq!(scalar.to_u128(), Ok(u128::MAX));
    assert_eq!(scalar.to_i128(), Err(ScalarError::OutOfRange));
    for text in ["340282366920938463463374607431768211456", "-1", "-0"] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, Schema::Yaml12)
                .unwrap()
                .to_u128(),
            Err(ScalarError::OutOfRange)
        );
    }
}

#[test]
fn non_decimal_integer_overflow_is_checked() {
    for (text, schema, expected) in [
        (
            "0xffffffffffffffffffffffffffffffff",
            Schema::Yaml12,
            u128::MAX,
        ),
        ("0b11111111", Schema::Yaml11, 255),
        ("377", Schema::Yaml12, 377),
        ("0377", Schema::Yaml11, 255),
        ("1:00:00", Schema::Yaml11, 3600),
    ] {
        assert_eq!(
            resolve(text, ScalarStyle::Plain, None, schema)
                .unwrap()
                .to_u128(),
            Ok(expected)
        );
    }
    let hex_overflow = "0x100000000000000000000000000000000";
    assert_eq!(
        resolve(hex_overflow, ScalarStyle::Plain, None, Schema::Yaml12)
            .unwrap()
            .to_u128(),
        Err(ScalarError::OutOfRange)
    );
    let signed_min = "-0x80000000000000000000000000000000";
    assert_eq!(
        resolve(signed_min, ScalarStyle::Plain, None, Schema::Yaml11)
            .unwrap()
            .to_i128(),
        Ok(i128::MIN)
    );
    let huge_base60 = format!("1{}", ":00".repeat(100));
    let scalar = resolve(&huge_base60, ScalarStyle::Plain, None, Schema::Yaml11).unwrap();
    assert_eq!(scalar.kind(), ScalarKind::Integer);
    assert_eq!(scalar.to_u128(), Err(ScalarError::OutOfRange));
}

#[test]
fn special_floats_are_supported_without_accepting_finite_overflow() {
    for schema in [Schema::Yaml12, Schema::Yaml11] {
        for text in [".inf", ".Inf", ".INF", "+.inf", "+.Inf", "+.INF"] {
            let scalar = resolve(text, ScalarStyle::Plain, None, schema).unwrap();
            assert_eq!(scalar.to_f64(), Ok(f64::INFINITY));
            assert_eq!(scalar.to_f32(), Ok(f32::INFINITY));
        }
        for text in ["-.inf", "-.Inf", "-.INF"] {
            let scalar = resolve(text, ScalarStyle::Plain, None, schema).unwrap();
            assert_eq!(scalar.to_f64(), Ok(f64::NEG_INFINITY));
            assert_eq!(scalar.to_f32(), Ok(f32::NEG_INFINITY));
        }
        for text in [".nan", ".NaN", ".NAN"] {
            let scalar = resolve(text, ScalarStyle::Plain, None, schema).unwrap();
            assert!(scalar.to_f64().unwrap().is_nan());
            assert!(scalar.to_f32().unwrap().is_nan());
        }
    }
    for text in ["1e9999", "-1e9999"] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
        assert_eq!(scalar.kind(), ScalarKind::Float);
        assert_eq!(scalar.to_f64(), Err(ScalarError::OutOfRange));
        assert_eq!(scalar.to_f32(), Err(ScalarError::OutOfRange));
    }
    let large = resolve("1e40", ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(large.to_f64(), Ok(1e40));
    assert_eq!(large.to_f32(), Err(ScalarError::OutOfRange));
}

#[test]
fn float_rounding_underflow_and_signed_zero_follow_ieee_conversion() {
    let scalar = resolve("0.1", ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(scalar.to_f64(), Ok(0.1_f64));
    assert_eq!(scalar.to_f32(), Ok(0.1_f32));
    // This decimal lies just above the midpoint between adjacent f32 values.
    // Going through f64 first loses that distinction and rounds down twice.
    let scalar = resolve(
        "1.000000059604644775390625000000000000000000001",
        ScalarStyle::Plain,
        None,
        Schema::Yaml12,
    )
    .unwrap();
    assert_eq!(scalar.to_f32(), Ok(f32::from_bits(1.0_f32.to_bits() + 1)));
    let scalar = resolve("1e-9999", ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(scalar.to_f64(), Ok(0.0));
    assert_eq!(scalar.to_f32(), Ok(0.0));
    for text in ["-0.0", "-1e-9999"] {
        let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
        assert_eq!(scalar.to_f64().unwrap().to_bits(), (-0.0_f64).to_bits());
        assert_eq!(scalar.to_f32().unwrap().to_bits(), (-0.0_f32).to_bits());
    }
}

#[test]
fn conversions_do_not_coerce_between_scalar_kinds() {
    let integer = resolve("42", ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    let mismatch = ScalarError::TypeMismatch {
        expected: ScalarKind::Float,
        actual: ScalarKind::Integer,
    };
    assert_eq!(integer.to_f64().unwrap_err(), mismatch);
    assert_eq!(integer.to_f32().unwrap_err(), mismatch);
    let float = resolve("42.0", ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(
        float.to_i128(),
        Err(ScalarError::TypeMismatch {
            expected: ScalarKind::Integer,
            actual: ScalarKind::Float
        })
    );
    assert_eq!(
        float.to_u128(),
        Err(ScalarError::TypeMismatch {
            expected: ScalarKind::Integer,
            actual: ScalarKind::Float
        })
    );
    let string = resolve("true", ScalarStyle::DoubleQuoted, None, Schema::Yaml12).unwrap();
    assert_eq!(
        string.to_bool(),
        Err(ScalarError::TypeMismatch {
            expected: ScalarKind::Boolean,
            actual: ScalarKind::String
        })
    );
    let null = resolve("null", ScalarStyle::Plain, None, Schema::Yaml12).unwrap();
    assert_eq!(
        null.to_i128(),
        Err(ScalarError::TypeMismatch {
            expected: ScalarKind::Integer,
            actual: ScalarKind::Null
        })
    );
}
