#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::derive_partial_eq_without_eq,
    clippy::similar_names,
    clippy::uninlined_format_args
)]

use indoc::indoc;
use serde::Deserialize;
use serde_json::Value;
use serde_saphyr::{Budget, Error, Options};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use crate::serde_yaml::adapt_to_miri;

fn test_de<T>(yaml: &str, expected: &T)
where
    T: serde::de::DeserializeOwned + PartialEq + Debug,
{
    let deserialized: T = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(*expected, deserialized);
}

#[test]
fn test_folded_block_scalar_bool() {
    let yaml = indoc! {"!!bool >-\n  true\n"};
    test_de(yaml, &true);
}

#[test]
fn test_folded_block_scalar_int() {
    let yaml = indoc! {"!!int >-\n  42\n"};
    test_de(yaml, &42_i32);
}

#[test]
fn test_folded_block_scalar_float() {
    let yaml = indoc! {"!!float >-\n  1.5\n"};
    test_de(yaml, &1.5_f64);
}

#[test]
fn test_folded_block_scalar_string() {
    let yaml = ">-\n  folded\n  block\n";
    test_de(yaml, &"folded block".to_owned());
}

#[test]
fn test_literal_block_scalar_string() {
    let yaml = "|-\n  literal\n  block\n";
    test_de(yaml, &"literal\nblock".to_owned());
}

/*
#[test]
fn test_borrowed() {
    let yaml = indoc! {"
        - plain nonàscii
        - 'single quoted'
        - \"double quoted\"
    "};
    let expected = vec!["plain nonàscii", "single quoted", "double quoted"];
    test_de(yaml, &expected);
}

#[test]
fn test_borrowed_struct() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct User<'a> {
        name: &'a str,
        email: &'a str,
    }

    let yaml = indoc! {"
        name: Alice
        email: alice@example.com
    "};

    let user: User<'_> = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(
        user,
        User {
            name: "Alice",
            email: "alice@example.com",
        }
    );

    let yaml_ptr = yaml.as_ptr() as usize;
    let yaml_end = yaml_ptr + yaml.len();
    let name_ptr = user.name.as_ptr() as usize;
    let email_ptr = user.email.as_ptr() as usize;
    assert!(name_ptr >= yaml_ptr && name_ptr < yaml_end);
    assert!(email_ptr >= yaml_ptr && email_ptr < yaml_end);
}
*/

#[test]
fn test_alias() {
    let yaml = indoc! {"
        first:
          &alias
          1
        second:
          *alias
        third: 3
    "};
    let mut expected = BTreeMap::new();
    expected.insert("first".to_owned(), 1);
    expected.insert("second".to_owned(), 1);
    expected.insert("third".to_owned(), 3);
    test_de(yaml, &expected);
}

#[test]
fn test_option() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: Option<f64>,
        b: Option<String>,
        c: Option<bool>,
    }
    let yaml = indoc! {"
        b:
        c: true
    "};
    let expected = Data {
        a: None,
        b: None,
        c: Some(true),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_option_alias() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: Option<f64>,
        b: Option<String>,
        c: Option<bool>,
        d: Option<f64>,
        e: Option<String>,
        f: Option<bool>,
    }
    let yaml = indoc! {"
        none_f:
          &none_f
          ~
        none_s:
          &none_s
          ~
        none_b:
          &none_b
          ~

        some_f:
          &some_f
          1.0
        some_s:
          &some_s
          x
        some_b:
          &some_b
          true

        a: *none_f
        b: *none_s
        c: *none_b
        d: *some_f
        e: *some_s
        f: *some_b
    "};
    let expected = Data {
        a: None,
        b: None,
        c: None,
        d: Some(1.0),
        e: Some("x".to_owned()),
        f: Some(true),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_enum_alias() {
    #[derive(Deserialize, PartialEq, Debug)]
    enum E {
        A,
        B(u8, u8),
    }
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: E,
        b: E,
    }

    let yaml = indoc! {"
        definitions:
            - &aref
              A:
            - &bref
              B:
                - 1
                - 2

        a: *aref
        b: *bref
    "};

    let expected = Data {
        a: E::A,
        b: E::B(1, 2),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_number_as_string() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Num {
        value: String,
    }
    let yaml = indoc! {"
        # Cannot be represented as u128
        value: 340282366920938463463374607431768211457
    "};
    let expected = Num {
        value: "340282366920938463463374607431768211457".to_owned(),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_number_as_string_small() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Num {
        value: String,
    }
    let yaml = indoc! {
        "value: 123"
    };
    let expected = Num {
        value: "123".to_owned(),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_bool_as_string() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Bool {
        value: String,
    }
    let yaml = indoc! {
        "value: true"
    };
    let expected = Bool {
        value: "true".to_owned(),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_empty_string() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Struct {
        empty: Option<String>,
        tilde: Option<String>,
        null: Option<String>,
    }
    let yaml = indoc! {"
        empty:
        tilde: ~
        \"null\": null
    "};
    let expected = Struct {
        empty: None,
        tilde: None,
        null: None,
    };
    test_de(yaml, &expected);
}

#[test]
fn test_i128_big() {
    let expected: i128 = i64::MIN as i128 - 1;
    let yaml = indoc! {"
        -9223372036854775809
    "};
    assert_eq!(expected, serde_saphyr::from_str(yaml).unwrap());

    let octal = indoc! {"
        -0o1000000000000000000001
    "};
    assert_eq!(expected, serde_saphyr::from_str(octal).unwrap());
}

#[test]
fn test_u128_big() {
    let expected: u128 = u64::MAX as u128 + 1;
    let yaml = indoc! {"
        18446744073709551616
    "};
    assert_eq!(expected, serde_saphyr::from_str(yaml).unwrap());

    let octal = indoc! {"
        0o2000000000000000000000
    "};
    assert_eq!(expected, serde_saphyr::from_str(octal).unwrap());
}

#[test]
fn test_number_alias_as_string() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Num {
        version: String,
        value: String,
    }
    let yaml = indoc! {"
        version: &a 1.10
        value: *a
    "};
    let expected = Num {
        version: "1.10".to_owned(),
        value: "1.10".to_owned(),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_byte_order_mark() {
    let yaml = "\u{feff}- 0\n";
    let expected = vec![0];
    test_de(yaml, &expected);
}

#[test]
fn test_bomb() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Data {
        expected: String,
    }

    // This would deserialize an astronomical number of elements if we were
    // vulnerable.
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
        j: &j [*i,*i,*i,*i,*i,*i,*i,*i,*i]
        k: &k [*j,*j,*j,*j,*j,*j,*j,*j,*j]
        l: &l [*k,*k,*k,*k,*k,*k,*k,*k,*k]
        m: &m [*l,*l,*l,*l,*l,*l,*l,*l,*l]
        n: &n [*m,*m,*m,*m,*m,*m,*m,*m,*m]
        o: &o [*n,*n,*n,*n,*n,*n,*n,*n,*n]
        p: &p [*o,*o,*o,*o,*o,*o,*o,*o,*o]
        q: &q [*p,*p,*p,*p,*p,*p,*p,*p,*p]
        r: &r [*q,*q,*q,*q,*q,*q,*q,*q,*q]
        s: &s [*r,*r,*r,*r,*r,*r,*r,*r,*r]
        t: &t [*s,*s,*s,*s,*s,*s,*s,*s,*s]
        u: &u [*t,*t,*t,*t,*t,*t,*t,*t,*t]
        v: &v [*u,*u,*u,*u,*u,*u,*u,*u,*u]
        w: &w [*v,*v,*v,*v,*v,*v,*v,*v,*v]
        x: &x [*w,*w,*w,*w,*w,*w,*w,*w,*w]
        y: &y [*x,*x,*x,*x,*x,*x,*x,*x,*x]
        z: &z [*y,*y,*y,*y,*y,*y,*y,*y,*y]
        expected: string
    "};
    // Budget breach
    let result: Result<Data, Error> = serde_saphyr::from_str_with_options(yaml, adapt_to_miri());
    assert!(result.is_err());
}

#[test]
fn test_numbers() {
    let cases = [
        ("0xF0", "240"),
        ("+0xF0", "240"),
        ("-0xF0", "-240"),
        ("0o70", "56"),
        ("+0o70", "56"),
        ("-0o70", "-56"),
        ("0b10", "2"),
        ("+0b10", "2"),
        ("-0b10", "-2"),
        ("127", "127"),
        ("+127", "127"),
        ("-127", "-127"),
        (".inf", ".inf"),
        (".Inf", ".inf"),
        (".INF", ".inf"),
        ("-.inf", "-.inf"),
        ("-.Inf", "-.inf"),
        ("-.INF", "-.inf"),
        (".nan", ".nan"),
        (".NaN", ".nan"),
        (".NAN", ".nan"),
        ("0.1", "0.1"),
    ];
    for &(yaml, expected) in &cases {
        let value = serde_saphyr::from_str::<Value>(yaml).unwrap();
        assert_eq!(
            value.to_string().trim_matches('"'),
            expected,
            "For YAML: {yaml}"
        );
    }

    // NOT numbers.
    let cases = [
        "++.inf", "+-.inf", "++1", "+-1", "-+1", "--1", "+--1", "0x+1", "0x-1", "-0x+1", "-0x-1",
        "++0x1", "+-0x1", "-+0x1", "--0x1",
    ];
    for yaml in &cases {
        let value = serde_saphyr::from_str::<Value>(yaml).unwrap();
        assert_eq!(value.to_string().trim_matches('"'), *yaml);
    }
}

#[test]
fn test_nan() {
    // There is no negative NaN in YAML.
    assert!(
        serde_saphyr::from_str::<f32>(".nan")
            .unwrap()
            .is_sign_positive()
    );
    assert!(
        serde_saphyr::from_str::<f64>(".nan")
            .unwrap()
            .is_sign_positive()
    );
}

#[test]
fn test_ignore_tag() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Data {
        struc: Struc,
        tuple: Tuple,
        newtype: Newtype,
        map: BTreeMap<char, usize>,
        vec: Vec<usize>,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Struc {
        x: usize,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Tuple(usize, usize);

    #[derive(Deserialize, Debug, PartialEq)]
    struct Newtype(usize);

    let yaml = indoc! {"
        struc: !wat
          x: 0
        tuple: !wat
          - 0
          - 0
        newtype: !wat 0
        map: !wat
          x: 0
        vec: !wat
          - 0
    "};

    let expected = Data {
        struc: Struc { x: 0 },
        tuple: Tuple(0, 0),
        newtype: Newtype(0),
        map: {
            let mut map = BTreeMap::new();
            map.insert('x', 0);
            map
        },
        vec: vec![0],
    };

    test_de(yaml, &expected);
}

#[test]
fn test_no_required_fields() {
    #[derive(Deserialize, PartialEq, Debug)]
    pub struct NoRequiredFields {
        optional: Option<usize>,
    }

    for document in ["", "# comment\n"] {
        let expected = NoRequiredFields { optional: None };
        let deserialized: NoRequiredFields = serde_saphyr::from_str(document).unwrap();
        assert_eq!(expected, deserialized);

        let expected = Vec::<String>::new();
        let deserialized: Vec<String> = serde_saphyr::from_str(document).unwrap();
        assert_eq!(expected, deserialized);

        let expected = BTreeMap::new();
        let deserialized: BTreeMap<char, usize> = serde_saphyr::from_str(document).unwrap();
        assert_eq!(expected, deserialized);

        let expected = None;
        let deserialized: Option<String> = serde_saphyr::from_str(document).unwrap();
        assert_eq!(expected, deserialized);

        let expected = Value::Null;
        let deserialized: Value = serde_saphyr::from_str(document).unwrap();
        assert_eq!(expected, deserialized);
    }
}

#[test]
fn test_empty_scalar() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Struct<T> {
        thing: T,
    }

    let empty_vector: Vec<String> = vec![];
    let empty_map: HashMap<String, String> = HashMap::new();

    let yaml = "thing:\n";
    let expected = Struct {
        thing: empty_vector,
    };
    test_de(yaml, &expected);

    let expected = Struct { thing: empty_map };
    test_de(yaml, &expected);
}

#[test]
fn test_python_safe_dump() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Frob {
        foo: u32,
    }

    // This matches output produced by PyYAML's `yaml.safe_dump` when using the
    // default_style parameter.
    //
    //    >>> import yaml
    //    >>> d = {"foo": 7200}
    //    >>> print(yaml.safe_dump(d, default_style="|"))
    //    "foo": !!int |-
    //      7200
    //
    let yaml = indoc! {r#"
        "foo": !!int |-
            7200
    "#};

    let expected = Frob { foo: 7200 };
    test_de(yaml, &expected);
}

#[test]
fn test_enum_untagged() {
    #[derive(Deserialize, PartialEq, Debug)]
    #[serde(untagged)]
    pub enum UntaggedEnum {
        A {
            r#match: bool,
        },
        AB {
            r#match: String,
        },
        B {
            #[serde(rename = "if")]
            r#match: bool,
        },
        C(String),
    }

    // A
    {
        let expected = UntaggedEnum::A { r#match: true };
        let deserialized: UntaggedEnum = serde_saphyr::from_str("match: True").unwrap();
        assert_eq!(expected, deserialized);
    }
    // AB
    {
        let expected = UntaggedEnum::AB {
            r#match: "T".to_owned(),
        };
        let deserialized: UntaggedEnum = serde_saphyr::from_str("match: T").unwrap();
        assert_eq!(expected, deserialized);
    }
    // B
    {
        let expected = UntaggedEnum::B { r#match: true };
        let deserialized: UntaggedEnum = serde_saphyr::from_str("if: True").unwrap();
        assert_eq!(expected, deserialized);
    }
    // C
    {
        let expected = UntaggedEnum::C("match".to_owned());
        let deserialized: UntaggedEnum = serde_saphyr::from_str("match").unwrap();
        assert_eq!(expected, deserialized);
    }
}
