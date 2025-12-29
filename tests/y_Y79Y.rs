use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
struct Doc {
    foo: String,
    bar: i64,
}

#[test]
fn yaml_y79y_block_scalar_with_tab() {
    let y = "foo: |\n\t\nbar: 1\n";
    let r: Result<Doc, Error> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Tabs cannot be used to indent block scalars");
}

#[test]
fn yaml_y79y_quoted_scalar_with_tab() {
    let y = "foo: \"\\t\\n\"\nbar: 1\n";
    let v: Doc = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.foo, "\t\n");
    assert_eq!(v.bar, 1);
}
