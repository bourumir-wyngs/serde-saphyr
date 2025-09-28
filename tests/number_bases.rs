use serde::Deserialize;

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
struct OnlyLegacy { legacy_u16: u16 }

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
