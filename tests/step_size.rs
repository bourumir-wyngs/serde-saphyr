#![cfg(all(feature = "serialize", feature = "deserialize"))]
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

    let serializer_options = serde_saphyr::ser_options! {
        indent_step: 7,
    };

    let mut serialized = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut serialized, &value, serializer_options).unwrap();

    let parsed: serde_json::Value = serde_saphyr::from_str(&serialized).unwrap();
    assert_eq!(parsed, value);
}

/// Nested block collections directly after a sequence dash (`- - a`, `- a:` with a nested
/// value) round-trip for every `indent_step`, with and without `compact_list_indent`.
#[test]
fn nested_collections_after_dash_round_trip_for_every_step_size() {
    let values = [
        serde_json::json!([["a", "b"]]),
        serde_json::json!([1, ["a", "b"]]),
        serde_json::json!({"k": [["a", "b"]]}),
        serde_json::json!([[{"a": 1, "b": 2}]]),
        serde_json::json!([{"a": ["x", "y"], "b": 2}]),
        serde_json::json!({"k": [{"a": {"b": 1}, "c": 2}]}),
    ];
    for indent_step in 1..=8 {
        for compact_list_indent in [true, false] {
            for value in &values {
                let options = serde_saphyr::ser_options! {
                    indent_step: indent_step,
                    compact_list_indent: compact_list_indent,
                };
                let yaml = serde_saphyr::to_string_with_options(value, options).unwrap();
                let parsed: serde_json::Value = serde_saphyr::from_str(&yaml).unwrap_or_else(|e| {
                    panic!("indent_step {indent_step}, compact {compact_list_indent}: {e}\n{yaml}")
                });
                assert_eq!(
                    &parsed, value,
                    "indent_step {indent_step}, compact {compact_list_indent}:\n{yaml}"
                );
            }
        }
    }
}

/// An anchored sequence after a dash (`- &a1` then the items on their own lines) keeps its
/// items one level deeper for every `indent_step`.
#[test]
fn anchored_sequence_after_dash_round_trips_for_every_step_size() {
    use serde_saphyr::RcAnchor;
    use std::rc::Rc;

    let shared = Rc::new(vec![1, 2]);
    let value = vec![RcAnchor(shared.clone()), RcAnchor(shared)];
    for indent_step in 1..=8 {
        let options = serde_saphyr::ser_options! { indent_step: indent_step };
        let yaml = serde_saphyr::to_string_with_options(&value, options).unwrap();
        let back: Vec<RcAnchor<Vec<i32>>> = serde_saphyr::from_str(&yaml)
            .unwrap_or_else(|e| panic!("indent_step {indent_step}: {e}\n{yaml}"));
        assert_eq!(*back[0].0, vec![1, 2], "indent_step {indent_step}:\n{yaml}");
        assert!(
            Rc::ptr_eq(&back[0].0, &back[1].0),
            "indent_step {indent_step}:\n{yaml}"
        );
    }
}
