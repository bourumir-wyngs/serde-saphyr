#![cfg(all(feature = "serialize", feature = "deserialize"))]
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct HasStrings {
    zero: String,
    xnan: String,
    colon: String,
    comment: String,
    ending_colon: String,
    trim_ending_colon: String,
}

#[test]
fn strings_that_look_special_are_quoted() -> Result<()> {
    let v = HasStrings {
        zero: "0".to_string(),
        xnan: "nan".to_string(),
        colon: "a: b".to_string(),
        comment: "# hi".to_string(),
        ending_colon: "hi:".to_string(),
        trim_ending_colon: "hey:\n".to_string(),
    };

    let out = serde_saphyr::to_string(&v).expect("serialize");

    // Each of these fields should be quoted or escaped so that they are preserved as strings
    // and do not get parsed as numbers, special floats, or mapping syntax.
    assert!(out.contains("zero: \"0\""), "'0' must be quoted: {out}");
    assert!(out.contains("xnan: \"nan\""), "'nan' must be quoted: {out}");
    assert!(out.contains("\"# hi\""), "comment must be quoted: {out}");
    assert!(
        out.contains("colon: \"a: b\""),
        "'a: b' must be quoted: {out}"
    );
    assert!(
        out.contains("ending_colon: \"hi:\""),
        "ending colon must be quoted: {out}"
    );
    // Multiline strings auto-select literal block style when the content is representable
    // (the trailing `:` is fine as block content). The round-trip below verifies that
    // the value is still preserved exactly.
    assert!(
        out.contains("trim_ending_colon: |\n  hey:\n"),
        "multiline ending colon should round-trip via literal block: {out}"
    );

    let r = serde_saphyr::from_str(&out)?;
    assert_eq!(v, r);
    Ok(())
}
