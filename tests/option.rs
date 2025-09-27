use serde::Deserialize;
use serde_saphyr::sf_serde;

#[derive(Debug, Deserialize, PartialEq)]
struct OptTest {
    none: Option<i32>,
    some: Option<i32>,
}

#[test]
fn option_fields_null_and_some() {
    // YAML where `none` is explicit null, `some` is a number
    let y1 = "none: null\nsome: 42\n";
    let o1: OptTest = sf_serde::from_str(y1).unwrap();
    assert_eq!(o1.none, None);
    assert_eq!(o1.some, Some(42));

    // YAML where `none` is tilde, `some` is a number
    let y2 = "none: ~\nsome: 123\n";
    let o2: OptTest = sf_serde::from_str(y2).unwrap();
    assert_eq!(o2.none, None);
    assert_eq!(o2.some, Some(123));

    // YAML where `none` is empty value, `some` is present
    let y3 = "none:\nsome: 99\n";
    let o3: OptTest = sf_serde::from_str(y3).unwrap();
    assert_eq!(o3.none, None);
    assert_eq!(o3.some, Some(99));

    // YAML where both are provided with numbers
    let y4 = "none: 1\nsome: 2\n";
    let o4: OptTest = sf_serde::from_str(y4).unwrap();
    assert_eq!(o4.none, Some(1));
    assert_eq!(o4.some, Some(2));
}
