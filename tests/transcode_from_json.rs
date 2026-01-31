#[cfg(test)]
mod tests {
    use serde::Deserialize;

    fn transcode_yaml_to_json(yaml: &str) -> String {
        serde_saphyr::with_deserializer_from_str(yaml, |de| {
            let v = serde_json::Value::deserialize(de)?;
            serde_json::to_string(&v).map_err(|e| serde::de::Error::custom(e.to_string()))
        })
        .unwrap()
    }

    fn assert_json_eq(actual_json: &str, expected_json: &str) {
        let actual: serde_json::Value = serde_json::from_str(actual_json).unwrap();
        let expected: serde_json::Value = serde_json::from_str(expected_json).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn from_sample_platter_json_to_yaml_without_options() {
        let json = r#"
            {
                "rainbow": ["red", "orange", "yellow", "green", "blue", "purple"],
                "point": {
                    "x": 12,
                    "xy": -34
                },
                "bools": [true, false]
            }
        "#;

        let mut json_deserializer = serde_json::Deserializer::from_str(json);

        let mut yaml = String::new();
        let mut yaml_serializer = serde_saphyr::Serializer::new(&mut yaml);
        serde_transcode::transcode(&mut json_deserializer, &mut yaml_serializer).unwrap();

        assert_eq!(
            yaml,
            r#"rainbow:
  - red
  - orange
  - yellow
  - green
  - blue
  - purple
point:
  x: 12
  xy: -34
bools:
  - true
  - false
"#
        );
    }

    #[test]
    fn from_nested_json_with_indent_step_option() {
        let json = r#"
            {
                "formats": [
                    {
                        "name": "CBOR",
                        "deacronymization": ["Concise", "Binary", "Object", "Representation"],
                        "self-describing": false
                    },
                    {
                        "name": "JSON",
                        "deacronymization": ["JavaScript", "Object", "Notation"],
                        "self-describing": true
                    },
                    {
                        "name": "YAML",
                        "deacronymization": ["YAML", "Ain't", "Markup", "Language"],
                        "self-describing": true
                    }
                ]
            }
        "#;

        let mut json_deserializer = serde_json::Deserializer::from_str(json);

        let mut yaml = String::new();
        let mut yaml_serializer_options = serde_saphyr::ser_options! {
            indent_step: 2,
        };
        let mut yaml_serializer =
            serde_saphyr::Serializer::with_options(&mut yaml, &mut yaml_serializer_options);
        serde_transcode::transcode(&mut json_deserializer, &mut yaml_serializer).unwrap();

        assert_eq!(
            yaml,
            r#"formats:
  - name: CBOR
    deacronymization:
      - Concise
      - Binary
      - Object
      - Representation
    self-describing: false
  - name: JSON
    deacronymization:
      - JavaScript
      - Object
      - Notation
    self-describing: true
  - name: YAML
    deacronymization:
      - YAML
      - Ain't
      - Markup
      - Language
    self-describing: true
"#
        );
    }

    #[test]
    fn from_sample_platter_yaml_to_json() {
        let yaml = r#"rainbow:
  - red
  - orange
  - yellow
  - green
  - blue
  - purple
point:
  x: 12
  xy: -34
bools:
  - true
  - false
"#;

        let json = transcode_yaml_to_json(yaml);

        assert_json_eq(
            &json,
            r#"{
              "rainbow": ["red", "orange", "yellow", "green", "blue", "purple"],
              "point": {"x": 12, "xy": -34},
              "bools": [true, false]
            }"#,
        );
    }

    #[test]
    fn from_nested_yaml_to_json() {
        let yaml = r#"formats:
  - name: CBOR
    deacronymization:
      - Concise
      - Binary
      - Object
      - Representation
    self-describing: false
  - name: JSON
    deacronymization:
      - JavaScript
      - Object
      - Notation
    self-describing: true
  - name: YAML
    deacronymization:
      - YAML
      - Ain't
      - Markup
      - Language
    self-describing: true
"#;

        let json = transcode_yaml_to_json(yaml);

        assert_json_eq(
            &json,
            r#"{
              "formats": [
                {
                  "name": "CBOR",
                  "deacronymization": ["Concise", "Binary", "Object", "Representation"],
                  "self-describing": false
                },
                {
                  "name": "JSON",
                  "deacronymization": ["JavaScript", "Object", "Notation"],
                  "self-describing": true
                },
                {
                  "name": "YAML",
                  "deacronymization": ["YAML", "Ain't", "Markup", "Language"],
                  "self-describing": true
                }
              ]
            }"#,
        );
    }
}
