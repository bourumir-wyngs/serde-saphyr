use serde_saphyr::SerializerOptions;

#[test]
fn changing_step_size_results_in_valid_yaml() {
    let value = serde_json::json!({
        "formats": [
            {
                "name": "CBOR",
                "deacronymization": ["Concise", "Binary", "Object", "Representation"],
                "self-describing": false,
            },
            {
                "name": "JSON",
                "deacronymization": ["JavaScript", "Object", "Notation"],
                "self-describing": true,
            },
            {
                "name": "YAML",
                "deacronymization": ["YAML", "Ain't", "Markup", "Language"],
                "self-describing": true,
            },
        ]
    });

    let serializer_options = SerializerOptions {
        indent_step: 7,
        ..Default::default()
    };

    let mut serialized = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut serialized, &value, serializer_options).unwrap();

    println!("{}", serialized);

    let parsed: serde_json::Value = serde_saphyr::from_str(&serialized).unwrap();
    assert_eq!(parsed, value);
}
