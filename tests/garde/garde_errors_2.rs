use serde::Deserialize;
use serde::Deserializer;

#[test]
fn main() {
    use garde::Validate;
    use std::collections::HashMap;

    #[derive(Clone, Debug, Deserialize, Validate)]
    pub struct BaseEntityProperties {
        #[garde(length(min = 3, max = 100))]
        id: Option<String>,

        #[garde(length(min = 3, max = 100))]
        name: String,
        #[garde(length(min = 3, max = 100))]
        platform: String,
    }

    impl BaseEntityProperties {
        pub fn get_object_id(&self) -> String {
            "example".to_string()
        }
    }

    #[derive(Clone, Debug, Deserialize, Validate)]
    #[garde(transparent)]
    pub struct BaseEntity {
        #[serde(flatten)]
        #[garde(dive)]
        pub default: BaseEntityProperties,
    }

    // Simplified deserializer: from a sequence of BaseEntity -> Option<HashMap<String, BaseEntity>>
    fn map_base_entity<'de, D>(de: D) -> Result<Option<HashMap<String, BaseEntity>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // If the YAML provides a sequence, deserialize it into a Vec first.
        let items = Vec::<BaseEntity>::deserialize(de)?;
        let mut map = HashMap::with_capacity(items.len());
        for item in items {
            let key = item.default.get_object_id();
            if map.insert(key.clone(), item).is_some() {
                return Err(serde::de::Error::custom(format!("Duplicate entry {}", key)));
            }
        }
        Ok(Some(map))
    }

    #[derive(Debug, Deserialize, Validate)]
    #[allow(dead_code)]
    struct Cfg {
        #[serde(default, deserialize_with = "map_base_entity")]
        #[garde(dive)]
        pub hash_map: Option<HashMap<String, BaseEntity>>,

        #[garde(dive)]
        pub array: Option<Vec<BaseEntity>>,
    }

    let examples_yamls = [
        r#"
array:
- name: "test"
        "#,
        r#"
hash_map:
- name: "test"
        "#,
        r#"
hash_map:
- platform: "ex"
  name: "test"
        "#,
    ];

    // Deserialization errors (like missing required fields) may not always carry usable spans.
    // The key case for this test is garde validation inside `deserialize_with`, where the
    // validation path may not exactly match a recorded YAML path.
    let should_render_snippet = [false, false, true];

    for (yaml, expect_snippet) in examples_yamls.into_iter().zip(should_render_snippet) {
        let err = serde_saphyr::from_str_valid::<Cfg>(yaml).expect_err("must fail");
        let rendered = err.to_string();
        println!("{rendered}");

        if expect_snippet {
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
    }
}