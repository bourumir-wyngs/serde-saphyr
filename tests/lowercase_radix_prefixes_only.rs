#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::{Error, Options, from_reader_with_options, from_str_with_options};

fn lowercase_only() -> Options {
    serde_saphyr::options! { lowercase_radix_prefixes_only: true }
}

fn assert_inferred(yaml: &str, expected: Value, options: Options) {
    let from_str: Value = from_str_with_options(yaml, options.clone()).unwrap();
    let from_reader: Value = from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(from_str, expected, "string input: {yaml}");
    assert_eq!(from_reader, expected, "reader input: {yaml}");
}

const UPPERCASE: [(&str, i64); 7] = [
    ("0XFF", 255),
    ("0Xff", 255),
    ("+0XFF", 255),
    ("-0X10", -16),
    ("0O17", 15),
    ("0B11", 3),
    ("-0B11", -3),
];

#[test]
fn uppercase_prefixes_are_integers_by_default() {
    assert!(!Options::default().lowercase_radix_prefixes_only);
    for (yaml, value) in UPPERCASE {
        assert_inferred(yaml, json!(value), Options::default());
        assert_eq!(
            from_str_with_options::<i64>(yaml, Options::default()).unwrap(),
            value
        );
    }
    assert_eq!(serde_saphyr::from_str::<u8>("0XFF").unwrap(), 255);
}

#[test]
fn uppercase_prefixes_are_strings_when_inferred() {
    for (yaml, _) in UPPERCASE {
        assert_inferred(yaml, json!(yaml), lowercase_only());
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(untagged)]
    enum Scalar {
        Int(i64),
        Text(String),
    }
    #[derive(Debug, Deserialize, PartialEq)]
    struct Holder {
        value: Scalar,
    }
    assert_eq!(
        from_str_with_options::<Holder>("value: 0XFF\n", lowercase_only())
            .unwrap()
            .value,
        Scalar::Text("0XFF".to_owned())
    );
    assert_eq!(
        from_str_with_options::<Holder>("value: 0xFF\n", lowercase_only())
            .unwrap()
            .value,
        Scalar::Int(255)
    );
    // String targets keep the text in either mode.
    assert_eq!(
        from_str_with_options::<String>("0XFF", lowercase_only()).unwrap(),
        "0XFF"
    );
}

#[test]
fn uppercase_prefixes_are_rejected_by_integer_targets() {
    for (yaml, _) in UPPERCASE {
        assert!(
            matches!(
                from_str_with_options::<i64>(yaml, lowercase_only())
                    .unwrap_err()
                    .without_snippet(),
                Error::InvalidScalar { ty: "i64", .. }
            ),
            "{yaml}"
        );
        assert!(from_str_with_options::<i32>(yaml, lowercase_only()).is_err());
        assert!(from_str_with_options::<i128>(yaml, lowercase_only()).is_err());
    }
    for ty_check in [
        from_str_with_options::<u8>("0XFF", lowercase_only()).map(u64::from),
        from_str_with_options::<u16>("0O17", lowercase_only()).map(u64::from),
        from_str_with_options::<u32>("0B11", lowercase_only()).map(u64::from),
        from_str_with_options::<u64>("+0XFF", lowercase_only()),
    ] {
        assert!(ty_check.is_err());
    }
    assert!(from_str_with_options::<u128>("0XFF", lowercase_only()).is_err());
    assert!(from_str_with_options::<Value>("!!int 0XFF", lowercase_only()).is_err());
}

#[test]
fn lowercase_prefixes_are_unaffected() {
    for (yaml, value) in [
        ("0xFF", 255),
        ("0xff", 255),
        ("-0x10", -16),
        ("0o17", 15),
        ("0b11", 3),
        ("17", 17),
    ] {
        assert_inferred(yaml, json!(value), lowercase_only());
        assert_eq!(
            from_str_with_options::<i64>(yaml, lowercase_only()).unwrap(),
            value
        );
    }
    assert_eq!(
        from_str_with_options::<u8>("0xFF", lowercase_only()).unwrap(),
        255
    );
    // Quoted and string-tagged scalars stay strings.
    assert_inferred("'0xFF'", json!("0xFF"), lowercase_only());
    assert_inferred("!!str 0XFF", json!("0XFF"), Options::default());
}
