#![allow(clippy::zero_sized_map_values)]

use indoc::indoc;
#[cfg(not(miri))]
use serde::de::{SeqAccess, Visitor};
use serde::Deserialize;

use serde_saphyr::{Deserializer, Value};
#[path = "utils/mod.rs"]
mod utils;
use utils::{test_error, deserializer_no_pathology};
#[cfg(not(miri))]
use std::collections::BTreeMap;
use std::collections::HashMap;
#[cfg(not(miri))]
use std::fmt;
use std::fmt::Debug;

#[test]
fn test_scan_error() {
    let yaml = ">\n@";
    let expected = "found character that cannot start any token at line 2 column 1, while scanning for the next token";
    test_error::<Value>(yaml, expected);
}

#[test]
fn test_incorrect_type() {
    let yaml = indoc! {"
        ---
        str
    "};
    let expected = "invalid type: string \"str\", expected i16 at line 2 column 1";
    test_error::<i16>(yaml, expected);
}

#[test]
fn test_incorrect_nested_type() {
    #[derive(Deserialize, Debug)]
    pub struct A {
        #[allow(dead_code)]
        pub b: Vec<B>,
    }
    #[derive(Deserialize, Debug)]
    pub enum B {
        C(#[allow(dead_code)] C),
    }
    #[derive(Deserialize, Debug)]
    pub struct C {
        #[allow(dead_code)]
        pub d: bool,
    }
    let yaml = indoc! {"
        b:
          - !C
            d: fase
    "};
    let expected = "b[0].d: invalid type: string \"fase\", expected a boolean at line 3 column 8";
    test_error::<A>(yaml, expected);
}

#[test]
fn test_empty() {
    let expected = "EOF while parsing a value";
    test_error::<String>("", expected);
}

#[test]
fn test_missing_field() {
    #[derive(Deserialize, Debug)]
    pub struct Basic {
        #[allow(dead_code)]
        pub v: bool,
        #[allow(dead_code)]
        pub w: bool,
    }
    let yaml = indoc! {"
        ---
        v: true
    "};
    let expected = "missing field `w` at line 2 column 1";
    test_error::<Basic>(yaml, expected);
}

#[test]
fn test_unknown_anchor() {
    let yaml = indoc! {"
        ---
        *some
    "};
    let expected = "reference to non existing anchor [some] at line 2 column 1";
    test_error::<String>(yaml, expected);
}

#[test]
fn test_ignored_unknown_anchor() {
    #[derive(Deserialize, Debug)]
    pub struct Wrapper {
        #[allow(dead_code)]
        pub c: (),
    }
    let yaml = indoc! {"
        b: [*This_anchor-is-unknown]
        c: ~
    "};
    let expected = "reference to non existing anchor [This_anchor-is-unknown] at line 1 column 5";
    test_error::<Wrapper>(yaml, expected);
}

#[test]
fn test_invalid_anchor_reference_message() {
    let yaml = "*invalid_anchor";
    let result: Result<Value, _> = serde_saphyr::from_str(yaml);
    match result {
        Ok(_) => panic!("Expected error for invalid anchor"),
        Err(e) => {
            assert_eq!(
                "reference to non existing anchor [invalid_anchor]",
                e.to_string()
            );
        }
    }
}

#[test]
fn test_bytes() {
    let yaml = "- 1\n- 2\n- 3\n";
    let bytes: Vec<u8> = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn test_two_documents() {
    let yaml = indoc! {"
        ---
        0
        ---
        1
    "};
    let expected = "deserializing from YAML containing more than one document is not supported";
    test_error::<usize>(yaml, expected);
}

#[test]
fn test_second_document_syntax_error() {
    let yaml = indoc! {"
        ---
        0
        ---
        ]
    "};

    let mut de = Deserializer::from_str(yaml);
    let first_doc = de.next().unwrap();
    let result = <usize as serde::Deserialize>::deserialize(first_doc);
    assert_eq!(0, result.unwrap());

    let second_doc = de.next().unwrap();
    let result = <usize as serde::Deserialize>::deserialize(second_doc);
    let expected =
        "did not find expected node content at line 4 column 1, while parsing a block node";
    assert_eq!(expected, result.unwrap_err().to_string());
}

#[test]
fn test_missing_enum_tag() {
    #[derive(Deserialize, Debug)]
    pub enum E {
        V(#[allow(dead_code)] usize),
    }
    let yaml = indoc! {r#"
        "V": 16
        "other": 32
    "#};
    let expected = "invalid length 2, expected map containing 1 entry";
    test_error::<E>(yaml, expected);
}

#[test]
fn test_deserialize_nested_enum() {
    #[derive(Deserialize, Debug, PartialEq)]
    pub enum Outer {
        Inner(#[allow(dead_code)] Inner),
    }
    #[derive(Deserialize, Debug, PartialEq)]
    pub enum Inner {
        Variant(#[allow(dead_code)] Vec<usize>),
    }

    let yaml = indoc! {
        "---\n!Inner []\n"
    };
    let result = Outer::deserialize(Deserializer::from_str(yaml));
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unknown variant"),
        "unexpected message: {}",
        msg
    );

    let yaml = indoc! {
        "---\n!Variant []\n"
    };
    let result = Outer::deserialize(Deserializer::from_str(yaml));
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unknown variant"),
        "unexpected message: {}",
        msg
    );
}

#[test]
fn test_variant_not_a_seq() {
    #[derive(Deserialize, Debug)]
    pub enum E {
        V(#[allow(dead_code)] usize),
    }
    let yaml = indoc! {"
        ---
        !V
        value: 0
    "};
    let expected = "invalid type: map, expected usize at line 2 column 1";
    test_error::<E>(yaml, expected);
}

#[test]
fn test_enum_mapping_has_extra_keys() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    enum Point {
        Tuple(u8, u8, u8),
        Struct { x: f64, y: f64 },
    }
    let yaml = indoc! {
        "
        Tuple:
          - 0
          - 0
          - 0
        Struct:
          x: 1.0
          y: 2.0
        "
    };
    // Single enum only expected here, not two
    let result = Point::deserialize(Deserializer::from_str(yaml));
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("invalid length 2, expected map containing 1 entry"),
        "unexpected message: {}",
        msg
    );
}

#[test]
fn test_enum_mapping_has_no_keys() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    enum Point {
        Struct { x: f64, y: f64 },
    }
    // This YAML has identation misplaced to Struct becomes an empty map
    let yaml = indoc! {
        "
        Struct:
        x: 1.0
        y: 2.0
        "
    };
    let result: Result<Point, _> = serde_saphyr::from_str(yaml);
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("invalid type: map, expected a leaf for empty enum, otherwise map naming the fields for enum"),
        "unexpected message: {}",
        msg
    );
}

#[test]
fn test_enum_mapping_has_no_keys_from_value() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    enum Point {
        Struct { x: f64, y: f64 },
    }
    // This YAML has identation misplaced to Struct becomes an empty map
    let yaml = indoc! {
        "
        Struct:
        x: 1.0
        y: 2.0
        "
    };
    let value: serde_saphyr::Value = serde_saphyr::from_str(yaml).unwrap();

    let result: Result<Point, _> = serde_saphyr::from_value(value.clone());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("invalid type: map, expected a leaf for empty enum, otherwise map naming the fields for enum"),
        "unexpected message: {}",
        msg
    );

    let result = Point::deserialize(&value);
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("invalid type: map, expected a leaf for empty enum, otherwise map naming the fields for enum"),
        "unexpected message: {}",
        msg
    );
}

#[test]
fn test_anchor_too_long() {
    use serde_saphyr::Value;
    const MAX: usize = 65_536;
    let long = "a".repeat(MAX + 1);
    let yaml = format!("&{long} 1\n");
    let expected = "unexpected tag error";
    test_error::<Value>(&yaml, expected);
}

#[test]
fn test_struct_from_sequence() {
    #[derive(Deserialize, Debug)]
    pub struct Struct {
        #[allow(dead_code)]
        pub x: usize,
        #[allow(dead_code)]
        pub y: usize,
    }
    let yaml = indoc! {"
        [0, 0]
    "};
    let expected = "invalid type: sequence, expected struct Struct";
    test_error::<Struct>(yaml, expected);
}

#[test]
fn test_bad_bool() {
    let yaml = indoc! {"
        ---
        !!bool str
    "};
    let expected = "invalid value: string \"str\", expected a boolean at line 2 column 1";
    test_error::<bool>(yaml, expected);
}

#[test]
fn test_bad_int() {
    let yaml = indoc! {"
        ---
        !!int str
    "};
    let expected = "invalid value: string \"str\", expected an integer at line 2 column 1";
    test_error::<i64>(yaml, expected);
}

#[test]
fn test_bad_float() {
    let yaml = indoc! {"
        ---
        !!float str
    "};
    let expected = "invalid value: string \"str\", expected a float at line 2 column 1";
    test_error::<f64>(yaml, expected);
}

#[test]
fn test_bad_null() {
    let yaml = indoc! {"
        ---
        !!null str
    "};
    let expected = "invalid value: string \"str\", expected null at line 2 column 1";
    test_error::<()>(yaml, expected);
}

#[test]
fn test_short_tuple() {
    let yaml = indoc! {"
        ---
        [0, 0]
    "};
    let expected = "invalid length 2, expected a tuple of size 3 at line 2 column 1";
    test_error::<(u8, u8, u8)>(yaml, expected);
}

#[test]
fn test_long_tuple() {
    let yaml = indoc! {"
        ---
        [0, 0, 0]
    "};
    let expected = "invalid length 3, expected sequence of 2 elements at line 2 column 1";
    test_error::<(u8, u8)>(yaml, expected);
}

#[test]
fn test_invalid_scalar_type() {
    #[derive(Deserialize, Debug)]
    pub struct S {
        #[allow(dead_code)]
        pub x: [i32; 1],
    }

    let yaml = "x: ''\n";
    let expected = "x: invalid type: string \"\", expected an array of length 1 at line 1 column 4";
    test_error::<S>(yaml, expected);
}

#[cfg(not(miri))]
#[test]
fn test_infinite_recursion_objects() {
    #[derive(Deserialize, Debug)]
    pub struct S {
        #[allow(dead_code)]
        pub x: Option<Box<S>>,
    }

    let yaml = "&a {'x': *a}";
    let expected = "recursion limit exceeded";
    test_error::<S>(yaml, expected);
}

#[cfg(not(miri))]
#[test]
fn test_infinite_recursion_arrays() {
    #[derive(Deserialize, Debug)]
    pub struct S(
        #[allow(dead_code)] pub usize,
        #[allow(dead_code)] pub Option<Box<S>>,
    );

    let yaml = "&a [0, *a]";
    let expected = "recursion limit exceeded";
    test_error::<S>(yaml, expected);
}

#[cfg(not(miri))]
#[test]
fn test_infinite_recursion_newtype() {
    #[derive(Deserialize, Debug)]
    pub struct S(#[allow(dead_code)] pub Option<Box<S>>);

    let yaml = "&a [*a]";
    let expected = "recursion limit exceeded";
    test_error::<S>(yaml, expected);
}

#[cfg(not(miri))]
#[test]
fn test_finite_recursion_objects() {
    #[derive(Deserialize, Debug)]
    pub struct S {
        #[allow(dead_code)]
        pub x: Option<Box<S>>,
    }

    let yaml = "{'x':".repeat(1_000) + &"}".repeat(1_000);
    let expected = "recursion limit exceeded at line 1 column 641";
    test_error::<S>(&yaml, expected);
}

#[cfg(not(miri))]
#[test]
fn test_finite_recursion_arrays() {
    #[derive(Deserialize, Debug)]
    pub struct S(
        #[allow(dead_code)] pub usize,
        #[allow(dead_code)] pub Option<Box<S>>,
    );

    let yaml = "[0, ".repeat(1_000) + &"]".repeat(1_000);
    let expected = "recursion limit exceeded at line 1 column 513";
    test_error::<S>(&yaml, expected);
}

#[cfg(not(miri))]
#[test]
fn test_billion_laughs() {
    #[derive(Debug)]
    struct X;

    impl<'de> Visitor<'de> for X {
        type Value = X;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("exponential blowup")
        }

        fn visit_unit<E>(self) -> Result<X, E> {
            Ok(X)
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<X, S::Error>
        where
            S: SeqAccess<'de>,
        {
            while let Some(X) = seq.next_element()? {}
            Ok(X)
        }
    }

    impl<'de> Deserialize<'de> for X {
        fn deserialize<D>(deserializer: D) -> Result<X, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(X)
        }
    }

    let yaml = indoc! {"
        a: &a ~
        b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]
        c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]
        d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]
        e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]
        f: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]
        g: &g [*f,*f,*f,*f,*f,*f,*f,*f,*f]
        h: &h [*g,*g,*g,*g,*g,*g,*g,*g,*g]
        i: &i [*h,*h,*h,*h,*h,*h,*h,*h,*h]
    "};
    let expected = "repetition limit exceeded";
    test_error::<BTreeMap<String, X>>(yaml, expected);
}

#[test]
fn test_duplicate_keys_cases() {
    let yaml = indoc! {"
        ---
        thing: true
        thing: false
    "};
    let expected = "duplicate entry with key \"thing\" (first defined at line 2 column 1) at line 3 column 1";
    test_error::<Value>(yaml, expected);

    let yaml = indoc! {"
        ---
        null: true
        ~: false
    "};
    let expected = "duplicate entry with null key at line 2 column 1";
    test_error::<Value>(yaml, expected);

    let yaml = indoc! {"
        ---
        99: true
        99: false
    "};
    let expected = "duplicate entry with key 99 (first defined at line 2 column 1) at line 3 column 1";
    test_error::<Value>(yaml, expected);

    let yaml = indoc! {"
        ---
        {}: true
        {}: false
    "};
    let expected = "duplicate entry in YAML map at line 2 column 1";
    test_error::<Value>(yaml, expected);
}

#[test]
fn test_duplicate_keys_hashmap() {
    use std::collections::HashMap;
    let yaml = indoc! {"\
        ---
        a: 1
        a: 2
    "};
    let expected = "duplicate entry with key \"a\" (first defined at line 2 column 1) at line 3 column 1";
    test_error::<HashMap<String, i32>>(yaml, expected);
}

#[test]
fn test_duplicate_keys_struct() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct S {
        a: i32,
    }
    let yaml = indoc! {"\
        ---
        a: 1
        a: 2
    "};
    let expected = "duplicate entry with key \"a\" (first defined at line 2 column 1) at line 3 column 1";
    test_error::<S>(yaml, expected);
}

#[test]
fn test_duplicate_key_error_message() {
    #[derive(Debug, Deserialize)]
    struct Data {
        data: HashMap<String, i32>,
    }

    let yaml_no_dups = "data:\n  key1: 1\n  key2: 2";
    match serde_saphyr::from_str::<Data>(yaml_no_dups) {
        Ok(data) => {
            assert_eq!(2, data.data.len());
            assert_eq!(1, *data.data.get("key1").unwrap());
            assert_eq!(2, *data.data.get("key2").unwrap());
        }
        Err(err) => assert_eq!(format!("{}", err), r#"Failes to parse valid YAML"#),
    }

    let yaml_dups = "data:\n  key: 1\n  key: 2";
    match serde_saphyr::from_str::<Data>(yaml_dups) {
        Ok(data) => panic!("Takes duplicate keys and returns {data:?}"),
        Err(err) => assert_eq!(
            format!("{}", err),
            r#"data: duplicate entry with key "key" (first defined at line 2 column 3) at line 3 column 3"#
        ),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TooLongTuple {
    a: (i32, i32, i32, i32), // Expects exactly four integers
}

#[test]
fn test_unexpected_end_of_sequence() {
    let input = "a: [1, 2, 3]";
    let result: Result<TooLongTuple, _> = serde_saphyr::from_str(input);

    match result {
        Ok(data) => panic!(
            "Deserialization of 3 member YAML list into 4 member tuple unexpectedly \
            succeeded with value: {:?}",
            data
        ),
        Err(e) => {
            let msg = e.to_string();
            println!("Error: {}", msg);
            assert_eq!(msg, "invalid length 3, expected a tuple of size 4");
        }
    }
}

#[derive(Deserialize)]
struct AliasTest {
    a: String,
    b: String,
}

#[test]
fn test_unresolved_alias() {
    // YAML input with an unresolved alias (&missing_anchor is not defined)
    let input = "
a: \"hello\"
b: *missing_anchor
";

    let result: Result<AliasTest, _> = serde_saphyr::from_str(input);

    match result {
        Ok(data) => panic!(
            "Deserialization unexpectedly succeeded with data: a={} and b={}",
            data.a, data.b
        ),
        Err(e) => {
            let msg = e.to_string();
            println!("Captured Error: {}", msg);
            assert!(
                msg.contains("reference to non existing anchor"),
                "Unexpected error message: '{}'",
                msg
            );
        }
    }
}

#[cfg(not(miri))]
#[test]
fn test_extreme_nesting_error_message() {
    // Construct YAML with extremely deep nesting to exceed the recursion limit.
    let yaml = "[".repeat(20_000) + &"]".repeat(20_000);
    let de = deserializer_no_pathology(&yaml);
    let result: Result<Value, _> = Value::deserialize(de);
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.starts_with("recursion limit exceeded"),
        "unexpected error: {}",
        msg
    );
}

#[cfg(not(miri))]
#[test]
fn test_long_alias_chain_error() {
    use std::fmt::Write;

    // Create a YAML document with a long chain of aliases. Each anchor
    // references the one defined immediately before it.
    let mut yaml = String::new();
    writeln!(&mut yaml, "k0: &a0 [1]").unwrap();
    for i in 1..150 {
        let curr_anchor = format!("a{}", i);
        let prev_anchor = format!("a{}", i - 1);
        writeln!(&mut yaml, "k{}: &{} [*{}]", i, curr_anchor, prev_anchor).unwrap();
    }
    yaml.push_str(&format!("final: *a{}", 149));

    let de = deserializer_no_pathology(&yaml);
    let result: Result<Value, _> = Value::deserialize(de);
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("recursion limit exceeded"),
        "unexpected error: {}",
        msg
    );
}

#[test]
fn test_error_location() {
    let result = serde_saphyr::from_str::<Value>("@invalid_yaml");
    let loc = result.unwrap_err().location().expect("location");
    assert_eq!(1, loc.line());
    assert_eq!(1, loc.column());
}

#[test]
fn test_error_location_expanded() {
    let yaml_input = r#"
key: valid
nested:
  - item1
  - item2
  - @invalid_yaml
valid_after_error: true
"#;

    let result = serde_saphyr::from_str::<Value>(yaml_input);
    let err = result.unwrap_err();
    let loc = err.location().expect("location should be provided");

    // Verify that the error is correctly reported at line 6, column 5 (start of "@invalid_yaml")
    assert_eq!(6, loc.line());
    assert_eq!(5, loc.column());
}

#[test]
fn test_error_filters_control_chars() {
    let yaml = "\x1b";
    let err = serde_saphyr::from_str::<Value>(yaml).unwrap_err();
    let msg = err.to_string();
    let has_ctrl = msg
        .bytes()
        .any(|b| (b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t') || b == 0x7f);
    assert!(
        !has_ctrl,
        "error message contains control characters: {:?}",
        msg
    );
}

#[test]
fn test_from_str_value_duplicate_location() {
    let yaml = "---\na: 1\na: 2\n";
    let err = serde_saphyr::from_str_value(yaml).unwrap_err();
    let loc = err.location().expect("location");
    assert_eq!(3, loc.line());
    assert_eq!(1, loc.column());
}

#[test]
fn test_from_str_value_unexpected_end_location() {
    let err = serde_saphyr::from_str_value("]").unwrap_err();
    let loc = err.location().expect("location");
    assert_eq!(1, loc.line());
    assert_eq!(1, loc.column());
}
