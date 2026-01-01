/*
  In Serde terms, *transcoding* means:
    - deserialize from one format (JSON)
    - immediately serialize into another format (YAML)
    - without building a Rust struct/enum in the middle

  Why?
  ----------------------
  - You have data in JSON and want to emit YAML.
  - You want to preserve the general structure without writing a schema.
  - You want streaming behavior (in general). In this particular example we
    use `from_str` for simplicity, but the same approach works with readers.

  How?
  -------------------------
  1) `serde_json::Deserializer` reads JSON and produces Serde "events".
  2) `serde_saphyr::Serializer` consumes the same events and writes YAML.
  3) `serde_transcode::transcode` connects the two.

  Serde is the common language in the middle.
  We are not converting JSON -> `serde_json::Value` -> YAML.
*/

fn main() {
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

    // JSON deserializer:
    //
    // `serde_json::Deserializer` implements Serde's `Deserializer` trait, which
    // yields the data in a generic way (maps, sequences, strings, numbers, ...).
    let mut json_deserializer = serde_json::Deserializer::from_str(json);

    // YAML serializer:
    //
    // `serde_saphyr::Serializer` implements Serde's `Serializer` trait and
    // writes YAML into any `fmt::Write` target (here: a `String`).
    let mut yaml = String::new();
    let mut yaml_serializer = serde_saphyr::Serializer::new(&mut yaml);

    // Bridge:
    serde_transcode::transcode(&mut json_deserializer, &mut yaml_serializer)
        .expect("transcoding JSON -> YAML must succeed for the sample input");

    // Print the YAML so you can see the result.
    //
    // Expected output:
    //
    // rainbow:
    //   - red
    //   - orange
    //   - yellow
    //   - green
    //   - blue
    //   - purple
    // point:
    //   x: 12
    //   y: -34
    // bools:
    //   - true
    //   - false
    print!("{yaml}");
}
