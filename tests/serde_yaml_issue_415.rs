#![cfg(feature = "deserialize")]

// Regression coverage for https://github.com/dtolnay/serde-yaml/issues/415.
// serde-saphyr accepts externally tagged enums in `{ Variant: value }` form.
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Ims {
    name: String,
    r#type: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Product {
    name: String,
    version: String,
    r#type: String,
}

#[derive(Debug, Deserialize, PartialEq)]
enum ImageBase {
    Ims { ims: Ims },
    Product { product: Product },
    ImageRef { image_ref: String },
}

#[derive(Debug, Deserialize, PartialEq)]
struct Cherry {
    name: String,
    base: ImageBase,
    configuration: Option<String>,
    configuration_group_names: Option<Vec<String>>,
    ref_name: Option<String>,
    description: Option<String>,
}

#[test]
fn singleton_map_struct_variants_deserialize() {
    // The issue's original YAML omitted the `Ims` variant discriminator and the
    // struct variant's `ims` field. Keeping both layers isolates the reported
    // map-vs-tag behavior from that separate data-model mismatch.
    let yaml = r#"
- name: my-name
  base:
    Ims:
      ims:
        name: my-name
        type: image
  configuration: my-configuration
  configuration_group_names:
  - Compute
- name: my-product
  base:
    Product:
      product:
        name: product-name
        version: "1.0"
        type: image
- name: my-reference
  base:
    ImageRef:
      image_ref: registry.example/image:latest
"#;

    let actual: Vec<Cherry> =
        serde_saphyr::from_str(yaml).expect("singleton-map enums should deserialize");

    assert_eq!(
        actual,
        vec![
            Cherry {
                name: "my-name".to_owned(),
                base: ImageBase::Ims {
                    ims: Ims {
                        name: "my-name".to_owned(),
                        r#type: "image".to_owned(),
                    },
                },
                configuration: Some("my-configuration".to_owned()),
                configuration_group_names: Some(vec!["Compute".to_owned()]),
                ref_name: None,
                description: None,
            },
            Cherry {
                name: "my-product".to_owned(),
                base: ImageBase::Product {
                    product: Product {
                        name: "product-name".to_owned(),
                        version: "1.0".to_owned(),
                        r#type: "image".to_owned(),
                    },
                },
                configuration: None,
                configuration_group_names: None,
                ref_name: None,
                description: None,
            },
            Cherry {
                name: "my-reference".to_owned(),
                base: ImageBase::ImageRef {
                    image_ref: "registry.example/image:latest".to_owned(),
                },
                configuration: None,
                configuration_group_names: None,
                ref_name: None,
                description: None,
            },
        ]
    );
}
