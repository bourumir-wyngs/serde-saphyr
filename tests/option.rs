use serde::de::Deserializer;
use serde::Deserialize;
use serde_saphyr::sf_serde;

#[derive(Debug, Deserialize, PartialEq)]
struct OptTest {
    none: Option<i32>,
    some: Option<i32>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct OptStringVecTest {
    double_string: Option<String>,
    single_string: Option<String>,
    #[serde(deserialize_with = "option_vec_from_string")]
    double_vec: Option<Vec<char>>,
    #[serde(deserialize_with = "option_vec_from_string")]
    single_vec: Option<Vec<char>>,
}

fn option_vec_from_string<'de, D>(deserializer: D) -> Result<Option<Vec<char>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
        .map(|opt| opt.map(|s| s.chars().collect::<Vec<_>>()))
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UnitHolder {
    unit: (),
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UnitStructHolder {
    unit: EmptyUnitStruct,
}

#[derive(Debug, Deserialize)]
struct EmptyUnitStruct;

#[derive(Debug, Deserialize, PartialEq)]
struct OptStringTest {
    plain: Option<String>,
    double: Option<String>,
    single: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NestedOptString {
    plain: Option<Option<String>>,
    quoted: Option<Option<String>>,
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

#[test]
fn option_string_respects_scalar_style() {
    let yaml = "plain:\ndouble: \"\"\nsingle: ''\n";
    let parsed: OptStringTest = sf_serde::from_str(yaml).unwrap();
    assert_eq!(parsed.plain, None);
    assert_eq!(parsed.double, Some(String::new()));
    assert_eq!(parsed.single, Some(String::new()));
}

#[test]
fn nested_option_string_respects_scalar_style() {
    let yaml = "plain:\nquoted: \"\"\n";
    let parsed: NestedOptString = sf_serde::from_str(yaml).unwrap();
    assert_eq!(parsed.plain, None);
    assert_eq!(parsed.quoted, Some(Some(String::new())));
}

#[test]
fn option_quoted_empty_strings_are_some() {
    let yaml = "double_string: \"\"\n\
single_string: ''\n\
double_vec: \"\"\n\
single_vec: ''\n";
    let parsed: OptStringVecTest = sf_serde::from_str(yaml).unwrap();

    assert_eq!(parsed.double_string.as_deref(), Some(""));
    assert_eq!(parsed.single_string.as_deref(), Some(""));
    let double_vec = parsed
        .double_vec
        .as_ref()
        .map(|v| v.iter().collect::<String>());
    let single_vec = parsed
        .single_vec
        .as_ref()
        .map(|v| v.iter().collect::<String>());
    assert_eq!(double_vec.as_deref(), Some(""));
    assert_eq!(single_vec.as_deref(), Some(""));
}

#[test]
fn unit_fields_reject_quoted_empty_strings() {
    for yaml in ["unit: \"\"", "unit: ''"] {
        let err = sf_serde::from_str::<UnitHolder>(yaml).unwrap_err();
        assert!(
            err.to_string().contains("unexpected value for unit"),
            "yaml: {yaml}, err: {err}"
        );
    }
}

#[test]
fn unit_struct_fields_reject_quoted_empty_strings() {
    for yaml in ["unit: \"\"", "unit: ''"] {
        let err = sf_serde::from_str::<UnitStructHolder>(yaml).unwrap_err();
        assert!(
            err.to_string().contains("unexpected value for unit"),
            "yaml: {yaml}, err: {err}"
        );
    }
}
