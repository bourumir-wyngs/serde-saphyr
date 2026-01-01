#[cfg(test)]
mod tests {
    use serde_saphyr::SerializerOptions;

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
                    "y": -34
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
  y: -34
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
        let mut yaml_serializer_options = SerializerOptions {
            indent_step: 2,
            ..Default::default()
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
  y: -34
bools:
  - true
  - false
"#;

        let v: serde_json::Value = serde_saphyr::from_str(yaml).unwrap();
        let json = serde_json::to_string(&v).unwrap();

        assert_json_eq(
            &json,
            r#"{
              "rainbow": ["red", "orange", "yellow", "green", "blue", "purple"],
              "point": {"x": 12, "y": -34},
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

        let v: serde_json::Value = serde_saphyr::from_str(yaml).unwrap();
        let json = serde_json::to_string(&v).unwrap();

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
