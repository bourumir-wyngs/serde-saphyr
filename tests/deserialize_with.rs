use serde::Deserialize;
use serde::Deserializer;

#[test]
fn deserialize_with_type_error_renders_snippet() {
    use std::collections::HashMap;

    #[derive(Clone, Debug, Deserialize)]
    #[allow(dead_code)]
    pub struct BaseEntityProperties {
        name: String,
        platform: String,
        count: i32,
    }

    impl BaseEntityProperties {
        pub fn get_object_id(&self) -> String {
            // Deterministic key for the test; the exact mapping key is irrelevant.
            "example".to_string()
        }
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct BaseEntity {
        #[serde(flatten)]
        pub default: BaseEntityProperties,
    }

    // Custom deserializer: from a YAML sequence of BaseEntity -> Option<HashMap<String, BaseEntity>>.
    fn map_base_entity<'de, D>(de: D) -> Result<Option<HashMap<String, BaseEntity>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let items = Vec::<BaseEntity>::deserialize(de)?;
        let mut map = HashMap::with_capacity(items.len());
        for item in items {
            let key = item.default.get_object_id();
            if map.insert(key.clone(), item).is_some() {
                return Err(serde::de::Error::custom(format!("Duplicate entry {key}")));
            }
        }
        Ok(Some(map))
    }

    #[derive(Debug, Deserialize)]
    struct Cfg {
        #[serde(default, deserialize_with = "map_base_entity")]
        #[allow(dead_code)]
        pub hash_map: Option<HashMap<String, BaseEntity>>,
    }

    // Trigger a plain Serde type error inside the `deserialize_with` inner Vec deserialization.
    let yaml = r#"
hash_map:
- platform: "linux"
  name: "test"
  count: !!int not-an-int
"#;

    let err = serde_saphyr::from_str::<Cfg>(yaml).expect_err("must fail");
    let rendered = err.to_string();
    println!("{rendered}");

    // We should render a rustc-like snippet block.
    assert!(
        rendered.contains(" -->"),
        "expected snippet header, got: {rendered}"
    );
    assert!(
        rendered.contains('^'),
        "expected span marker in snippet, got: {rendered}"
    );
}
