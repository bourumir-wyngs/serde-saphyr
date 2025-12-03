#[cfg(test)]
mod tests {
    use serde_saphyr::SerializerOptions;

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
        let mut yaml_serializer = serde_saphyr::YamlSer::new(&mut yaml);
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
            indent_step: 7,
            ..Default::default()
        };
        let mut yaml_serializer =
            serde_saphyr::YamlSer::with_options(&mut yaml, &mut yaml_serializer_options);
        serde_transcode::transcode(&mut json_deserializer, &mut yaml_serializer).unwrap();

        assert_eq!(
            yaml,
            // NOTE: serde_saphyr can't parse this yet; this will need to be changed after a fix for #31 is incorporated
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
}
