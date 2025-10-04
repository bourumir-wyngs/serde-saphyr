use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize, PartialEq)]
struct Numbers {
    hex_i32: i32,
    oct_i32: i32,
    bin_i8: i8,
    neg_hex: i64,
    neg_bin: i16,
    u_hex: u32,
    legacy_u16: u16,
}

#[test]
fn parse_numeric_bases_default() {
    // legacy_octal_numbers is false by default: 0052 is parsed as decimal 52
    let y = r#"
hex_i32: 0x2A
oct_i32: 0o52
bin_i8: 0b1010
neg_hex: -0x2A
neg_bin: -0b11
u_hex: 0xFF
legacy_u16: 0052
"#;
    let v: Numbers = serde_saphyr::from_str(y).expect("parse failed");
    assert_eq!(v.hex_i32, 42);
    assert_eq!(v.oct_i32, 42);
    assert_eq!(v.bin_i8, 10);
    assert_eq!(v.neg_hex, -42);
    assert_eq!(v.neg_bin, -3);
    assert_eq!(v.u_hex, 255);
    // legacy disabled: 0052 is decimal fifty-two
    assert_eq!(v.legacy_u16, 52);
}

#[derive(Debug, Deserialize, PartialEq)]
struct OnlyLegacy {
    legacy_u16: u16,
}

#[test]
fn parse_numeric_bases_with_legacy_octal() {
    let y = r#"
legacy_u16: 0052
"#;
    let mut opts = serde_saphyr::Options::default();
    opts.legacy_octal_numbers = true;
    let v: OnlyLegacy = serde_saphyr::from_str_with_options(y, opts).expect("parse failed");
    // With legacy octal enabled, 0052 is octal -> 42 decimal
    assert_eq!(v.legacy_u16, 42);
}

#[derive(Debug, Deserialize, PartialEq)]
struct LegacyZeroMixed {
    zero_u: u16,
    plus_zero_u: u16,
    neg_zero_i: i16,
}

#[test]
fn parse_legacy_octal_zero_variants() {
    let y = r#"
zero_u: 00
plus_zero_u: +00
neg_zero_i: -00
"#;
    let mut opts = serde_saphyr::Options::default();
    opts.legacy_octal_numbers = true;
    let v: LegacyZeroMixed = serde_saphyr::from_str_with_options(y, opts).expect("parse failed");
    assert_eq!(v.zero_u, 0);
    assert_eq!(v.plus_zero_u, 0);
    assert_eq!(v.neg_zero_i, 0);
}

#[test]
fn parse_legacy_octal_one() {
    let y = r#"
zero_u: 001
plus_zero_u: +001
neg_zero_i: -001
"#;
    let mut opts = serde_saphyr::Options::default();
    opts.legacy_octal_numbers = true;
    let v: LegacyZeroMixed = serde_saphyr::from_str_with_options(y, opts).expect("parse failed");
    assert_eq!(v.zero_u, 1);
    assert_eq!(v.plus_zero_u, 1);
    assert_eq!(v.neg_zero_i, -1);
}

#[test]
fn parse_legacy_octal_nine() {
    let y = r#"
zero_u: 009
plus_zero_u: +009
neg_zero_i: -009
"#;
    let mut opts = serde_saphyr::Options::default();
    opts.legacy_octal_numbers = true;
    let v: Result<LegacyZeroMixed, Error> = serde_saphyr::from_str_with_options(y, opts);
    assert!(v.is_err());
}

#[derive(Debug, Deserialize, PartialEq)]
struct UnderscoreNumbers {
    decimal_i64: i64,
    decimal_u64: u64,
    hex_u32: u32,
    bin_u8: u8,
}

#[test]
fn parse_numeric_literals_with_underscores() {
    let y = r#"
decimal_i64: -1_234_567_890
decimal_u64: 9_876_543_210
hex_u32: 0xAB_CD_EF_01
bin_u8: 0b1010_1010
"#;
    let v: UnderscoreNumbers = serde_saphyr::from_str(y).expect("parse failed");
    assert_eq!(v.decimal_i64, -1_234_567_890);
    assert_eq!(v.decimal_u64, 9_876_543_210);
    assert_eq!(v.hex_u32, 0xAB_CD_EF_01);
    assert_eq!(v.bin_u8, 0b1010_1010);
}

#[test]
fn parse_numeric_literals_with_invalid_digits() {
    let y = r#"
hex_u32: 0xABCDG
bin_u8: 0b1021
"#;
    let err = serde_saphyr::from_str::<UnderscoreNumbers>(y).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("invalid u32") || msg.contains("invalid u8"));
}
