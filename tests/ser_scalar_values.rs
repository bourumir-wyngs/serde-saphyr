#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Serialize;
use serde_saphyr::to_string;

#[test]
fn bool_value_in_struct_indented() {
    #[derive(Serialize)]
    struct S {
        flag: bool,
    }
    let yaml = to_string(&S { flag: true }).unwrap();
    assert!(yaml.contains("flag: true"), "yaml: {yaml}");
}

#[test]
fn i128_value_serialized() {
    let yaml = to_string(&i128::MAX).unwrap();
    assert!(
        yaml.contains("170141183460469231731687303715884105727"),
        "yaml: {yaml}"
    );
}

#[test]
fn u128_value_serialized() {
    let yaml = to_string(&u128::MAX).unwrap();
    assert!(
        yaml.contains("340282366920938463463374607431768211455"),
        "yaml: {yaml}"
    );
}

#[test]
fn char_value_serialized() {
    let yaml = to_string(&'A').unwrap();
    assert!(yaml.trim() == "A", "yaml: {yaml}");
}

#[test]
fn option_none_serialized_as_null() {
    let v: Option<i32> = None;
    let yaml = to_string(&v).unwrap();
    assert!(yaml.trim() == "null", "yaml: {yaml}");
}

#[test]
fn option_some_serialized_as_value() {
    let v: Option<i32> = Some(42);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.trim() == "42", "yaml: {yaml}");
}

#[test]
fn unit_serialized_as_null() {
    let yaml = to_string(&()).unwrap();
    assert!(yaml.trim() == "null", "yaml: {yaml}");
}

#[test]
fn i64_min_max_serialized() {
    let yaml_min = to_string(&i64::MIN).unwrap();
    let yaml_max = to_string(&i64::MAX).unwrap();
    assert!(
        yaml_min.contains("-9223372036854775808"),
        "yaml: {yaml_min}"
    );
    assert!(yaml_max.contains("9223372036854775807"), "yaml: {yaml_max}");
}

#[test]
fn u64_max_serialized() {
    let yaml = to_string(&u64::MAX).unwrap();
    assert!(yaml.contains("18446744073709551615"), "yaml: {yaml}");
}

#[test]
fn u128_as_map_value_not_at_line_start() {
    #[derive(Serialize)]
    struct S {
        val: u128,
    }
    let yaml = to_string(&S { val: u128::MAX }).unwrap();
    assert!(
        yaml.contains("340282366920938463463374607431768211455"),
        "yaml: {yaml}"
    );
}

#[test]
fn i128_as_map_value_not_at_line_start() {
    #[derive(Serialize)]
    struct S {
        val: i128,
    }
    let yaml = to_string(&S { val: i128::MIN }).unwrap();
    assert!(
        yaml.contains("-170141183460469231731687303715884105728"),
        "yaml: {yaml}"
    );
}

#[test]
fn bool_as_map_value_not_at_line_start() {
    #[derive(Serialize)]
    struct S {
        a: bool,
        b: bool,
    }
    let yaml = to_string(&S { a: true, b: false }).unwrap();
    assert!(
        yaml.contains("a: true") && yaml.contains("b: false"),
        "yaml: {yaml}"
    );
}

#[test]
fn serialize_i8_i16_i128_u128_f32_char_scalars() {
    assert_eq!(to_string(&42i8).unwrap(), "42\n");
    assert_eq!(to_string(&-1i16).unwrap(), "-1\n");
    assert_eq!(to_string(&999i128).unwrap(), "999\n");
    assert_eq!(to_string(&12345u128).unwrap(), "12345\n");
    let f32_yaml = to_string(&1.5f32).unwrap();
    assert!(f32_yaml.starts_with("1.5"), "f32: {f32_yaml}");
    assert_eq!(to_string(&'z').unwrap(), "z\n");
}

#[test]
fn unit_struct_serializes_as_null() {
    #[derive(Serialize)]
    struct Unit;
    let yaml = to_string(&Unit).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn option_none_serializes_as_null() {
    let v: Option<i32> = None;
    let yaml = to_string(&v).unwrap();
    assert_eq!(yaml, "null\n");
}

#[test]
fn option_some_serializes_value() {
    let v: Option<i32> = Some(5);
    let yaml = to_string(&v).unwrap();
    assert_eq!(yaml, "5\n");
}

#[test]
fn collect_str_via_display() {
    use serde::ser::Serializer;
    use std::fmt;

    struct DisplayOnly;

    impl fmt::Display for DisplayOnly {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("display-only-value")
        }
    }

    impl Serialize for DisplayOnly {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.collect_str(self)
        }
    }

    let yaml = to_string(&DisplayOnly).unwrap();
    assert!(yaml.contains("display-only-value"), "got: {yaml}");
}

#[test]
fn bool_at_line_start_in_seq() {
    let yaml = to_string(&vec![true, false]).unwrap();
    assert!(
        yaml.contains("- true") && yaml.contains("- false"),
        "got: {yaml}"
    );
}

#[test]
fn serialize_i8() {
    let s = serde_saphyr::to_string(&42i8).unwrap();
    assert_eq!(s.trim(), "42");
}

#[test]
fn serialize_i16() {
    let s = serde_saphyr::to_string(&1000i16).unwrap();
    assert_eq!(s.trim(), "1000");
}

#[test]
fn serialize_i128() {
    let s = serde_saphyr::to_string(&170141183460469231731687303715884105727i128).unwrap();
    assert!(s.trim().len() > 10);
}

#[test]
fn serialize_u8() {
    let s = serde_saphyr::to_string(&255u8).unwrap();
    assert_eq!(s.trim(), "255");
}

#[test]
fn serialize_u16() {
    let s = serde_saphyr::to_string(&65535u16).unwrap();
    assert_eq!(s.trim(), "65535");
}

#[test]
fn serialize_u128() {
    let s = serde_saphyr::to_string(&340282366920938463463374607431768211455u128).unwrap();
    assert!(s.trim().len() > 10);
}

#[test]
fn serialize_char() {
    let s = serde_saphyr::to_string(&'Z').unwrap();
    assert!(s.contains('Z'));
}

#[test]
fn serialize_unit() {
    let s = serde_saphyr::to_string(&()).unwrap();
    assert!(s.contains("null"));
}

#[test]
fn serialize_none() {
    let v: Option<i32> = None;
    let s = serde_saphyr::to_string(&v).unwrap();
    assert!(s.contains("null"));
}

#[test]
fn serialize_some() {
    let v: Option<i32> = Some(42);
    let s = serde_saphyr::to_string(&v).unwrap();
    assert_eq!(s.trim(), "42");
}

#[test]
fn serialize_unit_struct() {
    #[derive(Serialize)]
    struct Unit;
    let s = serde_saphyr::to_string(&Unit).unwrap();
    assert!(s.contains("null"));
}

#[test]
fn serialize_newtype_struct() {
    #[derive(Serialize)]
    struct Wrapper(i32);
    let s = serde_saphyr::to_string(&Wrapper(99)).unwrap();
    assert_eq!(s.trim(), "99");
}
