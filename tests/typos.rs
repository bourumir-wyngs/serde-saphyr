use serde::Deserialize;

#[test]
fn test_typo() {
    let yaml = r#"
packages:
 - name: somepkg
   alias:
      - other_name
 - name: wow
 - name: otherpkg
   alia:
      - other1
      - other2
    "#;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    pub struct Root {
        packages: Vec<Name>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    #[serde(deny_unknown_fields)]
    pub struct Name {
        name: String,
        alias: Option<Vec<String>>,
    }

    let err = serde_saphyr::from_str::<Root>(yaml).unwrap_err();
    let rendered = err.to_string();

    assert!(
        rendered.contains("unknown field `alia`"),
        "unexpected error output:\n{rendered}"
    );
    assert!(
        rendered.contains("--> <input>:8:4"),
        "unexpected error output:\n{rendered}"
    );
}