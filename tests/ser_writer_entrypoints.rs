#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Serialize;
use serde_saphyr::to_string_with_options;

#[test]
fn with_indent_constructor_produces_correct_indentation() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    #[derive(Serialize)]
    struct Outer {
        a: Inner,
    }
    let mut out = String::new();
    {
        let mut ser = serde_saphyr::Serializer::with_indent(&mut out, 4);
        Outer { a: Inner { x: 1 } }.serialize(&mut ser).unwrap();
    }
    // 4-space indent means "x" is indented by 4 spaces
    assert!(
        out.contains("    x:"),
        "expected 4-space indent, got:\n{out}"
    );
}

#[test]
fn to_io_writer_produces_yaml() {
    let mut buf = Vec::new();
    serde_saphyr::to_io_writer(&mut buf, &42i32).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("42"), "yaml: {s}");
}

#[test]
fn to_fmt_writer_produces_yaml() {
    let mut s = String::new();
    serde_saphyr::to_fmt_writer(&mut s, &"hello").unwrap();
    assert!(s.contains("hello"), "yaml: {s}");
}

#[test]
fn serializer_new_constructor() {
    let mut out = String::new();
    let mut ser = serde_saphyr::Serializer::new(&mut out);
    42i32.serialize(&mut ser).unwrap();
    assert!(out.contains("42"), "out: {out}");
}

#[test]
fn with_indent_changes_indentation() {
    #[derive(Serialize)]
    struct S<'a> {
        d: &'a str,
    }

    #[derive(Serialize)]
    struct E<'a> {
        a: &'a S<'a>,
    }
    let opts = serde_saphyr::ser_options! { indent_step: 4 };
    let s = S { d: "abc" };
    let e = E { a: &s };
    let yaml = to_string_with_options(&e, opts).unwrap();
    // With 4-space indent, the list item should be indented by 4 spaces
    assert!(
        yaml.contains("    d: abc"),
        "expected 4-space indent: {yaml}"
    );
}

#[test]
#[allow(deprecated)]
fn deprecated_to_writer() {
    let mut buf = String::new();
    serde_saphyr::to_writer(&mut buf, &42i32).unwrap();
    assert_eq!(buf.trim(), "42");
}

#[test]
#[allow(deprecated)]
fn deprecated_to_writer_with_options() {
    let mut buf = String::new();
    serde_saphyr::to_writer_with_options(&mut buf, &"hello", serde_saphyr::ser_options! {})
        .unwrap();
    assert!(buf.contains("hello"));
}

#[test]
fn to_io_writer_basic() {
    let mut buf = Vec::new();
    serde_saphyr::to_io_writer(&mut buf, &true).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.trim(), "true");
}

#[test]
fn to_io_writer_with_options_basic() {
    let mut buf = Vec::new();
    serde_saphyr::to_io_writer_with_options(
        &mut buf,
        &vec![1, 2, 3],
        serde_saphyr::ser_options! {},
    )
    .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("- 1"));
}

#[test]
fn to_io_writer_propagates_serializer_error_when_writer_succeeds() {
    struct Fails;

    impl Serialize for Fails {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("intentional serializer failure"))
        }
    }

    let mut buf = Vec::new();
    let err = serde_saphyr::to_io_writer(&mut buf, &Fails).unwrap_err();
    assert!(err.to_string().contains("intentional serializer failure"));
}

#[test]
fn to_fmt_writer_basic() {
    let mut buf = String::new();
    serde_saphyr::to_fmt_writer(&mut buf, &"test").unwrap();
    assert!(buf.contains("test"));
}

#[test]
fn serialize_with_custom_indent() {
    #[derive(Serialize)]
    struct Doc {
        items: Vec<i32>,
    }
    let opts = serde_saphyr::ser_options! {
        indent_step: 4,
    };
    let s = serde_saphyr::to_string_with_options(&Doc { items: vec![1, 2] }, opts).unwrap();
    assert!(s.contains("items:"));
}
