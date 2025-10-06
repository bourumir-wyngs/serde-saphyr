use indoc::indoc;
use serde::Deserialize as Derive;
use serde_saphyr::Error;
use std::collections::BTreeMap;
use std::fmt::Debug;

#[derive(Derive, Debug)]
#[allow(dead_code)]
enum Node {
    Unit,
    List(Vec<Node>),
}

#[cfg(not(miri))]
#[test]
#[ignore]
fn test_enum_billion_laughs_with_tags() {
    let yaml = indoc! {
        "
        a: &a !Unit
        b: &b !List [*a,*a,*a,*a,*a,*a,*a,*a,*a]
        c: &c !List [*b,*b,*b,*b,*b,*b,*b,*b,*b]
        d: &d !List [*c,*c,*c,*c,*c,*c,*c,*c,*c]
        e: &e !List [*d,*d,*d,*d,*d,*d,*d,*d,*d]
        f: &f !List [*e,*e,*e,*e,*e,*e,*e,*e,*e]
        g: &g !List [*f,*f,*f,*f,*f,*f,*f,*f,*f]
        h: &h !List [*g,*g,*g,*g,*g,*g,*g,*g,*g]
        i: &i !List [*h,*h,*h,*h,*h,*h,*h,*h,*h]
        "
    };
    let expected = "repetition limit exceeded";
    let parsed: Result<BTreeMap<String, String>, Error> = serde_saphyr::from_str(&yaml);
    assert!(parsed.is_err());
    println!("{}", parsed.unwrap_err());
    //assert!(format!("{}", parsed.unwrap_err()).contains("repetition limit exceeded"));
}

#[cfg(not(miri))]
#[test]
fn test_enum_billion_laughs() {
    let yaml = indoc! {
        "
        a: &a unit
        b: &b  [*a,*a,*a,*a,*a,*a,*a,*a,*a]
        c: &c  [*b,*b,*b,*b,*b,*b,*b,*b,*b]
        d: &d  [*c,*c,*c,*c,*c,*c,*c,*c,*c]
        e: &e  [*d,*d,*d,*d,*d,*d,*d,*d,*d]
        f: &f  [*e,*e,*e,*e,*e,*e,*e,*e,*e]
        g: &g  [*f,*f,*f,*f,*f,*f,*f,*f,*f]
        h: &h  [*g,*g,*g,*g,*g,*g,*g,*g,*g]
        i: &i  [*h,*h,*h,*h,*h,*h,*h,*h,*h]
        "
    };
    let expected = "repetition limit exceeded";
    let parsed: Result<BTreeMap<String, String>, Error> = serde_saphyr::from_str(&yaml);
    assert!(parsed.is_err());
    println!("{}", parsed.unwrap_err());
    //assert!(format!("{}", parsed.unwrap_err()).contains("repetition limit exceeded"));
}
