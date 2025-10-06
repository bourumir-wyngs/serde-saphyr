use indoc::indoc;
use serde::Deserialize as Derive;
use std::collections::BTreeMap;
use std::fmt::Debug;

#[path = "utils/mod.rs"]
mod utils;
use utils::test_error;

#[derive(Derive, Debug)]
#[allow(dead_code)]
enum Node {
    Unit,
    List(Vec<Node>),
}

#[cfg(not(miri))]
#[test]
fn test_enum_billion_laughs() {
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
    test_error::<BTreeMap<String, Node>>(yaml, expected);
}

