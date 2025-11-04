use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct HasStrings {
    zero: String,
    nan: String,
    colon: String,
    comment: String,
}

#[test]
fn strings_that_look_special_are_quoted() -> Result<()>{
    let v = HasStrings {
        zero: "0".to_string(),
        nan: "nan".to_string(),
        colon: "a: b".to_string(),
        comment: "# hi".to_string()
    };

    let out = serde_saphyr::to_string(&v).expect("serialize");

    // Each of these fields should be quoted or escaped so that they are preserved as strings
    // and do not get parsed as numbers, special floats, or mapping syntax.
    assert!(out.contains("zero: \"0\""), "'0' must be quoted: {out}");
    assert!(out.contains("nan: \"nan\""), "'nan' must be quoted: {out}");
    assert!(out.contains("\"# hi\""), "comment must be quoted: {out}");
    assert!(out.contains("colon: \"a: b\""), "'a: b' must be quoted: {out}");

    let r = serde_saphyr::from_str(&out)?;
    assert_eq!(v, r);
    Ok(())
}
