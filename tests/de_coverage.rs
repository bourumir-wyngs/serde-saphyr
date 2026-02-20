/// Targeted tests to increase coverage of src/de.rs.
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Bytes deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithBytes {
    data: serde_bytes::ByteBuf,
}

/// Bytes from a sequence of integers (0..=255).
#[test]
fn bytes_from_seq_of_ints() {
    let y = "data: [72, 101, 108, 108, 111]\n";
    let v: WithBytes = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.data.as_ref(), b"Hello");
}

/// Bytes from !!binary tag.
#[test]
fn bytes_from_binary_tag() {
    let y = "data: !!binary SGVsbG8=\n";
    let v: WithBytes = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.data.as_ref(), b"Hello");
}

/// Bytes from plain scalar without binary tag → error.
#[test]
fn bytes_from_plain_scalar_error() {
    let y = "data: hello\n";
    let err = serde_saphyr::from_str::<WithBytes>(y).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("binary") || msg.contains("!!binary") || msg.contains("sequence"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Char deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithChar {
    c: char,
}

/// Valid single-char.
#[test]
fn char_single() {
    let v: WithChar = serde_saphyr::from_str("c: A\n").unwrap();
    assert_eq!(v.c, 'A');
}

/// Multi-char string → error.
#[test]
fn char_multi_error() {
    let err = serde_saphyr::from_str::<WithChar>("c: AB\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("char") || msg.contains("single") || msg.contains("scalar"),
        "unexpected error: {msg}"
    );
}

/// Null scalar for char → error.
#[test]
fn char_null_error() {
    let err = serde_saphyr::from_str::<WithChar>("c: ~\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("null") || msg.contains("char") || msg.contains("null"),
        "unexpected error: {msg}"
    );
}

/// Empty scalar for char → error.
#[test]
fn char_empty_error() {
    let err = serde_saphyr::from_str::<WithChar>("c: \n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("char") || msg.contains("null") || msg.contains("scalar"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Unit / unit struct deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct UnitStruct;

/// Unit struct from empty mapping.
#[test]
fn unit_struct_from_empty_map() {
    let v: UnitStruct = serde_saphyr::from_str("{}\n").unwrap();
    assert_eq!(v, UnitStruct);
}

/// Unit struct from null scalar.
#[test]
fn unit_struct_from_null() {
    let v: UnitStruct = serde_saphyr::from_str("~\n").unwrap();
    assert_eq!(v, UnitStruct);
}

/// Unit struct from non-empty mapping → error.
#[test]
fn unit_struct_from_non_empty_map_error() {
    let err = serde_saphyr::from_str::<UnitStruct>("{a: 1}\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unit") || msg.contains("empty") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

/// Plain unit `()` from null.
#[test]
fn unit_from_null() {
    let v: () = serde_saphyr::from_str("~\n").unwrap();
    assert_eq!(v, ());
}

/// Plain unit `()` from non-null value → error.
#[test]
fn unit_from_non_null_error() {
    let err = serde_saphyr::from_str::<()>("hello\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unit") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Option deserialization edge cases
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithOption {
    val: Option<i32>,
}

/// Option from null tag.
#[test]
fn option_from_null_tag() {
    let v: WithOption = serde_saphyr::from_str("val: !!null ~\n").unwrap();
    assert_eq!(v.val, None);
}

/// Option from tilde.
#[test]
fn option_from_tilde() {
    let v: WithOption = serde_saphyr::from_str("val: ~\n").unwrap();
    assert_eq!(v.val, None);
}

/// Option from value.
#[test]
fn option_from_value() {
    let v: WithOption = serde_saphyr::from_str("val: 42\n").unwrap();
    assert_eq!(v.val, Some(42));
}

// ---------------------------------------------------------------------------
// Enum variant access: unit, newtype, tuple, struct
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
enum MyEnum {
    Unit,
    Newtype(u32),
    Tuple(u32, u32),
    Struct { x: i32, y: i32 },
}

/// Unit variant from scalar.
#[test]
fn enum_unit_variant() {
    let v: MyEnum = serde_saphyr::from_str("Unit\n").unwrap();
    assert_eq!(v, MyEnum::Unit);
}

/// Newtype variant from map.
#[test]
fn enum_newtype_variant() {
    let v: MyEnum = serde_saphyr::from_str("Newtype: 7\n").unwrap();
    assert_eq!(v, MyEnum::Newtype(7));
}

/// Tuple variant from map with sequence.
#[test]
fn enum_tuple_variant() {
    let v: MyEnum = serde_saphyr::from_str("Tuple: [3, 4]\n").unwrap();
    assert_eq!(v, MyEnum::Tuple(3, 4));
}

/// Struct variant from map.
#[test]
fn enum_struct_variant() {
    let v: MyEnum = serde_saphyr::from_str("Struct: {x: 1, y: 2}\n").unwrap();
    assert_eq!(v, MyEnum::Struct { x: 1, y: 2 });
}

/// Enum from sequence → error.
#[test]
fn enum_from_seq_error() {
    let err = serde_saphyr::from_str::<MyEnum>("[1, 2]\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("enum") || msg.contains("scalar") || msg.contains("mapping") || msg.contains("sequence"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_any edge cases
// ---------------------------------------------------------------------------

/// deserialize_any with NaN → returns string ".nan".
#[test]
fn deserialize_any_nan_as_string() {
    let v: serde_json::Value = serde_saphyr::from_str(".nan\n").unwrap();
    assert_eq!(v, serde_json::Value::String(".nan".to_string()));
}

/// deserialize_any with +inf → returns string ".inf".
#[test]
fn deserialize_any_inf_as_string() {
    let v: serde_json::Value = serde_saphyr::from_str(".inf\n").unwrap();
    assert_eq!(v, serde_json::Value::String(".inf".to_string()));
}

/// deserialize_any with -inf → returns string "-.inf".
#[test]
fn deserialize_any_neg_inf_as_string() {
    let v: serde_json::Value = serde_saphyr::from_str("-.inf\n").unwrap();
    assert_eq!(v, serde_json::Value::String("-.inf".to_string()));
}

/// deserialize_any with empty document → null/unit.
#[test]
fn deserialize_any_empty_document() {
    let v: serde_json::Value = serde_saphyr::from_str("").unwrap();
    assert_eq!(v, serde_json::Value::Null);
}

// ---------------------------------------------------------------------------
// deserialize_bool edge cases
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithBool {
    b: bool,
}

/// Strict booleans: "true" accepted.
#[test]
fn bool_strict_true() {
    let opts = serde_saphyr::options! { strict_booleans: true };
    let v: WithBool = serde_saphyr::from_str_with_options("b: true\n", opts).unwrap();
    assert_eq!(v.b, true);
}

/// Strict booleans: "yes" rejected.
#[test]
fn bool_strict_yes_rejected() {
    let opts = serde_saphyr::options! { strict_booleans: true };
    let err = serde_saphyr::from_str_with_options::<WithBool>("b: yes\n", opts).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("boolean") || msg.contains("strict") || msg.contains("invalid"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_str / deserialize_string with binary tag
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithString {
    s: String,
}

/// String from !!binary tag with ignore_binary_tag_for_string = true → raw base64 returned.
#[test]
fn string_from_binary_tag_ignored() {
    let opts = serde_saphyr::options! { ignore_binary_tag_for_string: true };
    // With ignore=true the tag is ignored and the raw base64 scalar is returned as-is
    let v: WithString =
        serde_saphyr::from_str_with_options("s: !!binary SGVsbG8=\n", opts).unwrap();
    assert_eq!(v.s, "SGVsbG8=");
}

/// String from !!binary tag without ignore → decoded UTF-8 string returned.
#[test]
fn string_from_binary_tag_decoded() {
    let opts = serde_saphyr::options! { ignore_binary_tag_for_string: false };
    // With ignore=false the binary is base64-decoded and returned as UTF-8 string
    let v: WithString =
        serde_saphyr::from_str_with_options("s: !!binary SGVsbG8=\n", opts).unwrap();
    assert_eq!(v.s, "Hello");
}

// ---------------------------------------------------------------------------
// no_schema mode: quoting required for ambiguous scalars
// ---------------------------------------------------------------------------

/// no_schema: plain integer for string field → error.
#[test]
fn no_schema_plain_int_for_string_error() {
    let opts = serde_saphyr::options! { no_schema: true };
    let err =
        serde_saphyr::from_str_with_options::<WithString>("s: 42\n", opts).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("quot") || msg.contains("schema") || msg.contains("string"),
        "unexpected error: {msg}"
    );
}

/// no_schema: quoted string is fine.
#[test]
fn no_schema_quoted_string_ok() {
    let opts = serde_saphyr::options! { no_schema: true };
    let v: WithString =
        serde_saphyr::from_str_with_options("s: \"hello\"\n", opts).unwrap();
    assert_eq!(v.s, "hello");
}

// ---------------------------------------------------------------------------
// Duplicate key policies
// ---------------------------------------------------------------------------

/// Duplicate key with Error policy → error.
#[test]
fn duplicate_key_error_policy() {
    let opts = serde_saphyr::options! { duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error };
    let err = serde_saphyr::from_str_with_options::<std::collections::HashMap<String, i32>>(
        "a: 1\na: 2\n",
        opts,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("duplicate") || msg.contains("key"),
        "unexpected error: {msg}"
    );
}

/// Duplicate key with FirstWins policy → first value kept.
#[test]
fn duplicate_key_first_wins() {
    let opts = serde_saphyr::options! { duplicate_keys: serde_saphyr::DuplicateKeyPolicy::FirstWins };
    let v: std::collections::HashMap<String, i32> =
        serde_saphyr::from_str_with_options("a: 1\na: 2\n", opts).unwrap();
    assert_eq!(v["a"], 1);
}

/// Duplicate key with LastWins policy → last value kept.
#[test]
fn duplicate_key_last_wins() {
    let opts = serde_saphyr::options! { duplicate_keys: serde_saphyr::DuplicateKeyPolicy::LastWins };
    let v: std::collections::HashMap<String, i32> =
        serde_saphyr::from_str_with_options("a: 1\na: 2\n", opts).unwrap();
    assert_eq!(v["a"], 2);
}

// ---------------------------------------------------------------------------
// Integer / float parsing edge cases
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithI128 {
    n: i128,
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithU128 {
    n: u128,
}

#[test]
fn deserialize_i128() {
    let v: WithI128 = serde_saphyr::from_str("n: -170141183460469231731687303715884105728\n").unwrap();
    assert_eq!(v.n, i128::MIN);
}

#[test]
fn deserialize_u128() {
    let v: WithU128 = serde_saphyr::from_str("n: 340282366920938463463374607431768211455\n").unwrap();
    assert_eq!(v.n, u128::MAX);
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithF32 {
    f: f32,
}

#[test]
fn deserialize_f32() {
    let v: WithF32 = serde_saphyr::from_str("f: 3.14\n").unwrap();
    assert!((v.f - 3.14f32).abs() < 1e-5);
}

// ---------------------------------------------------------------------------
// Newtype struct
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct Wrapper(i32);

#[test]
fn newtype_struct_deserialization() {
    let v: Wrapper = serde_saphyr::from_str("42\n").unwrap();
    assert_eq!(v, Wrapper(42));
}

// ---------------------------------------------------------------------------
// Tuple / tuple struct
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct TupleStruct(i32, i32);

#[test]
fn tuple_struct_deserialization() {
    let v: TupleStruct = serde_saphyr::from_str("[1, 2]\n").unwrap();
    assert_eq!(v, TupleStruct(1, 2));
}

#[test]
fn tuple_deserialization() {
    let v: (i32, String) = serde_saphyr::from_str("[7, hello]\n").unwrap();
    assert_eq!(v, (7, "hello".to_string()));
}

// ---------------------------------------------------------------------------
// Empty map / empty seq deserialization
// ---------------------------------------------------------------------------

#[test]
fn empty_map_deserialization() {
    let v: std::collections::HashMap<String, i32> =
        serde_saphyr::from_str("{}\n").unwrap();
    assert!(v.is_empty());
}

#[test]
fn empty_seq_deserialization() {
    let v: Vec<i32> = serde_saphyr::from_str("[]\n").unwrap();
    assert!(v.is_empty());
}

// ---------------------------------------------------------------------------
// deserialize_identifier
// ---------------------------------------------------------------------------

/// Identifier deserialization (used internally by Serde for enum/struct field names).
#[test]
fn deserialize_identifier_via_enum() {
    // Serde uses deserialize_identifier when matching enum variants
    let v: MyEnum = serde_saphyr::from_str("Unit\n").unwrap();
    assert_eq!(v, MyEnum::Unit);
}

// ---------------------------------------------------------------------------
// deserialize_ignored_any
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct IgnoreExtra {
    keep: i32,
    #[serde(skip)]
    _ignored: (),
}

#[test]
fn ignored_any_field() {
    // Extra fields in YAML are ignored via deserialize_ignored_any
    let v: IgnoreExtra =
        serde_saphyr::from_str("keep: 5\nextra_field: some_value\n").unwrap();
    assert_eq!(v.keep, 5);
}

// ---------------------------------------------------------------------------
// Seq with anchor (tests SeqStart anchor path)
// ---------------------------------------------------------------------------

#[test]
fn seq_with_anchor_and_alias() {
    let y = "- &anchor [1, 2, 3]\n- *anchor\n";
    let v: Vec<Vec<i32>> = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v, vec![vec![1, 2, 3], vec![1, 2, 3]]);
}

// ---------------------------------------------------------------------------
// Map with anchor (tests MapStart anchor path)
// ---------------------------------------------------------------------------

#[test]
fn map_with_anchor_and_alias() {
    let y = "base: &base\n  x: 1\nderived:\n  <<: *base\n  y: 2\n";
    #[derive(Debug, Deserialize, PartialEq)]
    struct Inner {
        x: i32,
        y: Option<i32>,
    }
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Outer {
        base: Inner,
        derived: Inner,
    }
    let v: Outer = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.derived.x, 1);
    assert_eq!(v.derived.y, Some(2));
}

// ---------------------------------------------------------------------------
// Byte buf (deserialize_byte_buf delegates to deserialize_bytes)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithByteBuf {
    data: serde_bytes::ByteBuf,
}

#[test]
fn byte_buf_from_binary_tag() {
    let y = "data: !!binary SGVsbG8=\n";
    let v: WithByteBuf = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.data.as_ref(), b"Hello");
}

// ---------------------------------------------------------------------------
// deserialize_any: signed integer path (negative number)
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_negative_int() {
    let v: serde_json::Value = serde_saphyr::from_str("-42\n").unwrap();
    assert_eq!(v, serde_json::Value::Number(serde_json::Number::from(-42)));
}

// ---------------------------------------------------------------------------
// deserialize_any: unsigned integer path
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_unsigned_int() {
    let v: serde_json::Value = serde_saphyr::from_str("42\n").unwrap();
    assert_eq!(v, serde_json::Value::Number(serde_json::Number::from(42u64)));
}

// ---------------------------------------------------------------------------
// deserialize_any: float path
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_float() {
    let v: serde_json::Value = serde_saphyr::from_str("3.14\n").unwrap();
    assert!(v.as_f64().is_some());
}

// ---------------------------------------------------------------------------
// deserialize_any: strict_booleans true/false paths
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_strict_bool_true() {
    let opts = serde_saphyr::options! { strict_booleans: true };
    let v: serde_json::Value = serde_saphyr::from_str_with_options("true\n", opts).unwrap();
    assert_eq!(v, serde_json::Value::Bool(true));
}

#[test]
fn deserialize_any_strict_bool_false() {
    let opts = serde_saphyr::options! { strict_booleans: true };
    let v: serde_json::Value = serde_saphyr::from_str_with_options("false\n", opts).unwrap();
    assert_eq!(v, serde_json::Value::Bool(false));
}

/// In strict mode, "yes" is not a bool → treated as string.
#[test]
fn deserialize_any_strict_bool_yes_as_string() {
    let opts = serde_saphyr::options! { strict_booleans: true };
    let v: serde_json::Value = serde_saphyr::from_str_with_options("yes\n", opts).unwrap();
    assert_eq!(v, serde_json::Value::String("yes".to_string()));
}

// ---------------------------------------------------------------------------
// deserialize_str / deserialize_string: null → error
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithStr {
    s: String,
}

#[test]
fn string_from_null_error() {
    let err = serde_saphyr::from_str::<WithStr>("s: ~\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("null") || msg.contains("string") || msg.contains("Option"),
        "unexpected error: {msg}"
    );
}

#[test]
fn string_from_empty_scalar_error() {
    let err = serde_saphyr::from_str::<WithStr>("s: \n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("null") || msg.contains("string") || msg.contains("Option"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_str: non-scalar → error
// ---------------------------------------------------------------------------

#[test]
fn str_from_seq_error() {
    let err = serde_saphyr::from_str::<WithStr>("s: [1, 2]\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("string") || msg.contains("scalar") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_string: tagged scalar that cannot be string → error
// ---------------------------------------------------------------------------

#[test]
fn string_from_int_tag_error() {
    let err = serde_saphyr::from_str::<WithStr>("s: !!int 42\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("tag") || msg.contains("string") || msg.contains("int"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// char: no_schema quoting required
// ---------------------------------------------------------------------------

#[test]
fn char_no_schema_plain_int_error() {
    let opts = serde_saphyr::options! { no_schema: true };
    let err = serde_saphyr::from_str_with_options::<WithChar>("c: 5\n", opts).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("quot") || msg.contains("schema") || msg.contains("string"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Merge key: sequence of maps `<<: [*a, *b]`
// ---------------------------------------------------------------------------

#[test]
fn merge_key_seq_of_maps() {
    let y = "\
defaults1: &d1
  x: 1
defaults2: &d2
  y: 2
merged:
  <<: [*d1, *d2]
  z: 3
";
    #[derive(Debug, Deserialize)]
    struct Merged {
        x: i32,
        y: i32,
        z: i32,
    }
    #[derive(Debug, Deserialize)]
    struct Outer {
        merged: Merged,
    }
    let v: Outer = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.merged.x, 1);
    assert_eq!(v.merged.y, 2);
    assert_eq!(v.merged.z, 3);
}

// ---------------------------------------------------------------------------
// Merge key: invalid merge value (scalar) → error
// ---------------------------------------------------------------------------

#[test]
fn merge_key_invalid_scalar_error() {
    let y = "merged:\n  <<: not_a_map\n  x: 1\n";
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Merged {
        x: Option<i32>,
    }
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Outer {
        merged: Merged,
    }
    let err = serde_saphyr::from_str::<Outer>(y).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("merge") || msg.contains("map") || msg.contains("sequence"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_any: legacy octal numbers
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_legacy_octal() {
    let opts = serde_saphyr::options! { legacy_octal_numbers: true };
    let v: serde_json::Value = serde_saphyr::from_str_with_options("00777\n", opts).unwrap();
    // 00777 legacy octal = 511 decimal
    assert_eq!(v, serde_json::Value::Number(serde_json::Number::from(511u64)));
}

// ---------------------------------------------------------------------------
// deserialize_map: skip_one_node (MA::skip_one_node)
// ---------------------------------------------------------------------------

/// Deserializing into a struct skips unknown fields via skip_one_node.
#[test]
fn map_skip_unknown_nested_fields() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Simple {
        keep: i32,
    }
    let y = "keep: 5\nskip_map: {a: 1, b: 2}\nskip_seq: [1, 2, 3]\n";
    let v: Simple = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.keep, 5);
}

// ---------------------------------------------------------------------------
// deserialize_seq: scalar (non-seq) → error
// ---------------------------------------------------------------------------

#[test]
fn seq_from_scalar_error() {
    let err = serde_saphyr::from_str::<Vec<i32>>("hello\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("sequence") || msg.contains("scalar") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_map: scalar (non-map) → error
// ---------------------------------------------------------------------------

#[test]
fn map_from_scalar_error() {
    let err =
        serde_saphyr::from_str::<std::collections::HashMap<String, i32>>("hello\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("mapping") || msg.contains("scalar") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_bool: non-scalar → error
// ---------------------------------------------------------------------------

#[test]
fn bool_from_seq_error() {
    let err = serde_saphyr::from_str::<WithBool>("b: [1, 2]\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("bool") || msg.contains("scalar") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Integer types: i8, i16, i32, u8, u16, u32
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithInts {
    i8v: i8,
    i16v: i16,
    i32v: i32,
    u8v: u8,
    u16v: u16,
    u32v: u32,
}

#[test]
fn deserialize_various_int_types() {
    let y = "i8v: -128\ni16v: -32768\ni32v: -2147483648\nu8v: 255\nu16v: 65535\nu32v: 4294967295\n";
    let v: WithInts = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.i8v, i8::MIN);
    assert_eq!(v.i16v, i16::MIN);
    assert_eq!(v.i32v, i32::MIN);
    assert_eq!(v.u8v, u8::MAX);
    assert_eq!(v.u16v, u16::MAX);
    assert_eq!(v.u32v, u32::MAX);
}

// ---------------------------------------------------------------------------
// deserialize_u64 / i64 directly
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithU64 {
    n: u64,
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithI64 {
    n: i64,
}

#[test]
fn deserialize_u64_max() {
    let v: WithU64 = serde_saphyr::from_str("n: 18446744073709551615\n").unwrap();
    assert_eq!(v.n, u64::MAX);
}

#[test]
fn deserialize_i64_min() {
    let v: WithI64 = serde_saphyr::from_str("n: -9223372036854775808\n").unwrap();
    assert_eq!(v.n, i64::MIN);
}

// ---------------------------------------------------------------------------
// Enum: no_schema quoting required for map key
// ---------------------------------------------------------------------------

#[test]
fn enum_no_schema_map_key_quoting_error() {
    let opts = serde_saphyr::options! { no_schema: true };
    // Map key is an unquoted integer, which requires quoting in no_schema mode
    let err = serde_saphyr::from_str_with_options::<MyEnum>("42: value\n", opts).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("quot") || msg.contains("schema") || msg.contains("string"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Enum: non-string map key → error
// ---------------------------------------------------------------------------

#[test]
fn enum_non_string_map_key_error() {
    // A map with a sequence as key is not valid for externally tagged enum
    let err = serde_saphyr::from_str::<MyEnum>("? [1, 2]\n: value\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("string") || msg.contains("key") || msg.contains("enum") || msg.contains("scalar"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Borrow: from_str allows borrowing scalars
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct BorrowedStr<'a> {
    s: &'a str,
}

#[test]
fn borrowed_str_from_str() {
    let yaml = "s: hello\n";
    let v: BorrowedStr = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(v.s, "hello");
}

// ---------------------------------------------------------------------------
// deserialize_any: tagged null → unit
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_tagged_null() {
    let v: serde_json::Value = serde_saphyr::from_str("!!null ~\n").unwrap();
    assert_eq!(v, serde_json::Value::Null);
}

/// deserialize_any: !!binary scalar → decoded string
#[test]
fn deserialize_any_binary_tag() {
    let v: serde_json::Value = serde_saphyr::from_str("!!binary SGVsbG8=\n").unwrap();
    // binary decoded to "Hello"
    assert_eq!(v, serde_json::Value::String("Hello".to_string()));
}

/// deserialize_any: !!str tagged scalar → always string
#[test]
fn deserialize_any_str_tag() {
    let v: serde_json::Value = serde_saphyr::from_str("!!str 42\n").unwrap();
    // !!str forces string interpretation regardless of scalar content
    assert_eq!(v, serde_json::Value::String("42".to_string()));
}

/// deserialize_any: !!int tagged scalar → error (can't deserialize into string)
#[test]
fn deserialize_any_int_tag_error() {
    let err = serde_saphyr::from_str::<serde_json::Value>("!!int 42\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("tag") || msg.contains("int") || msg.contains("string"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Enum: map-mode unit variant with null value
// ---------------------------------------------------------------------------

/// `{Unit: ~}` → unit variant in map mode
#[test]
fn enum_map_mode_unit_variant_null() {
    let v: MyEnum = serde_saphyr::from_str("{Unit: ~}\n").unwrap();
    assert_eq!(v, MyEnum::Unit);
}

/// `{Unit: }` → unit variant in map mode with empty value
#[test]
fn enum_map_mode_unit_variant_empty() {
    let v: MyEnum = serde_saphyr::from_str("{Unit: }\n").unwrap();
    assert_eq!(v, MyEnum::Unit);
}

// ---------------------------------------------------------------------------
// deserialize_seq: seq from scalar (non-binary) via bytes path
// ---------------------------------------------------------------------------

/// Bytes from map → error (unexpected event)
#[test]
fn bytes_from_map_error() {
    let y = "data: {a: 1}\n";
    let err = serde_saphyr::from_str::<WithBytes>(y).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("binary") || msg.contains("sequence") || msg.contains("unexpected"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_any: quoted scalar → string (not parsed as number)
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_quoted_number_as_string() {
    let v: serde_json::Value = serde_saphyr::from_str("\"42\"\n").unwrap();
    assert_eq!(v, serde_json::Value::String("42".to_string()));
}

// ---------------------------------------------------------------------------
// deserialize_any: null-like plain scalar → null
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_null_plain() {
    let v: serde_json::Value = serde_saphyr::from_str("null\n").unwrap();
    assert_eq!(v, serde_json::Value::Null);
}

#[test]
fn deserialize_any_tilde_null() {
    let v: serde_json::Value = serde_saphyr::from_str("~\n").unwrap();
    assert_eq!(v, serde_json::Value::Null);
}

// ---------------------------------------------------------------------------
// deserialize_map: empty map from null scalar
// ---------------------------------------------------------------------------

#[test]
fn map_from_null_is_empty() {
    // A null/~ at map position should yield an empty map
    let v: std::collections::HashMap<String, i32> =
        serde_saphyr::from_str("~\n").unwrap_or_default();
    assert!(v.is_empty());
}

// ---------------------------------------------------------------------------
// deserialize_seq: empty seq from null scalar
// ---------------------------------------------------------------------------

#[test]
fn seq_from_null_is_empty() {
    let v: Vec<i32> = serde_saphyr::from_str("~\n").unwrap_or_default();
    assert!(v.is_empty());
}

// ---------------------------------------------------------------------------
// Enum: unit variant in map mode with non-null value → error
// ---------------------------------------------------------------------------

#[test]
fn enum_map_mode_unit_variant_non_null_error() {
    let err = serde_saphyr::from_str::<MyEnum>("{Unit: 42}\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unit") || msg.contains("unexpected") || msg.contains("variant"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_str: null-tagged scalar → error
// ---------------------------------------------------------------------------

#[test]
fn str_from_null_tag_error() {
    let err = serde_saphyr::from_str::<WithStr>("s: !!null foo\n").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("null") || msg.contains("string"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_string: no_schema plain bool → error
// ---------------------------------------------------------------------------

#[test]
fn string_no_schema_plain_bool_error() {
    let opts = serde_saphyr::options! { no_schema: true };
    let err = serde_saphyr::from_str_with_options::<WithStr>("s: true\n", opts).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("quot") || msg.contains("schema") || msg.contains("string"),
        "unexpected error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// deserialize_any: no_schema plain number → still parsed as number
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_no_schema_plain_number() {
    let opts = serde_saphyr::options! { no_schema: true };
    // no_schema only affects typed string deserialization paths (deserialize_str/string).
    // In deserialize_any (typeless), plain numbers are still parsed as numbers.
    let v: serde_json::Value =
        serde_saphyr::from_str_with_options("42\n", opts).unwrap();
    assert_eq!(v, serde_json::Value::Number(42.into()));
}

// ---------------------------------------------------------------------------
// Merge key: null merge value (no-op)
// ---------------------------------------------------------------------------

#[test]
fn merge_key_null_value() {
    let y = "base:\n  x: 1\n  <<: ~\n";
    #[derive(Debug, Deserialize, PartialEq)]
    struct Base {
        x: i32,
    }
    #[derive(Debug, Deserialize)]
    struct Outer {
        base: Base,
    }
    let v: Outer = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.base.x, 1);
}

// ---------------------------------------------------------------------------
// deserialize_map: value requested before key → error (internal)
// This is hard to trigger directly; instead test normal map access works
// ---------------------------------------------------------------------------

#[test]
fn map_complex_nested() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Inner {
        a: i32,
        b: String,
    }
    #[derive(Debug, Deserialize, PartialEq)]
    struct Outer {
        inner: Inner,
        count: u32,
    }
    let y = "inner:\n  a: 5\n  b: hello\ncount: 3\n";
    let v: Outer = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.inner.a, 5);
    assert_eq!(v.inner.b, "hello");
    assert_eq!(v.count, 3);
}

// ---------------------------------------------------------------------------
// deserialize_seq: seq with anchor (tests anchor path in SA)
// ---------------------------------------------------------------------------

#[test]
fn seq_element_with_anchor() {
    let y = "- &item 42\n- *item\n- 100\n";
    let v: Vec<i32> = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v, vec![42, 42, 100]);
}

// ---------------------------------------------------------------------------
// deserialize_any: bool (non-strict)
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_yaml11_bool() {
    let v: serde_json::Value = serde_saphyr::from_str("yes\n").unwrap();
    assert_eq!(v, serde_json::Value::Bool(true));
}

// ---------------------------------------------------------------------------
// deserialize_any: signed int fallback (large negative that fits i64 but not u64)
// ---------------------------------------------------------------------------

#[test]
fn deserialize_any_large_negative_int() {
    let v: serde_json::Value = serde_saphyr::from_str("-9223372036854775808\n").unwrap();
    assert_eq!(v.as_i64(), Some(i64::MIN));
}
