//! Schema selection takes precedence over the compatibility option fields.

#[cfg(feature = "deserialize")]
mod deserialize {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};
    use serde_saphyr::scalar::Schema;
    use serde_saphyr::{Options, from_str_with_options, options};

    #[test]
    fn defaults_keep_the_legacy_scalar_policy() {
        let options = Options::default();
        assert_eq!(options.schema, Schema::Legacy);
        assert_eq!(
            from_str_with_options::<Value>("[yes, 010]", options).unwrap(),
            json!([true, 10.0])
        );
    }

    #[test]
    #[allow(deprecated)] // Exercise callers migrating one compatibility flag at a time.
    fn a_single_legacy_flag_keeps_the_other_flags_default() {
        let strict = options! { strict_booleans: true };
        assert_eq!(strict.schema, Schema::Legacy);
        assert_eq!(
            from_str_with_options::<Value>("[yes, 010]", strict).unwrap(),
            json!(["yes", 10.0])
        );

        let octal = options! { legacy_octal_numbers: true };
        assert_eq!(octal.schema, Schema::Legacy);
        assert_eq!(
            from_str_with_options::<Value>("[yes, 010]", octal).unwrap(),
            json!([true, 8])
        );
    }

    #[test]
    #[allow(deprecated)] // Old fields remain accepted but cannot override a schema.
    fn explicit_schema_wins_regardless_of_macro_field_order() {
        let schema = serde_saphyr::specific! { strict_booleans: true };
        for options in [
            options! {
                schema: schema,
                strict_booleans: false,
                legacy_octal_numbers: true,
            },
            options! {
                strict_booleans: false,
                legacy_octal_numbers: true,
                schema: schema,
            },
        ] {
            assert_eq!(
                from_str_with_options::<Value>("[yes, 010]", options).unwrap(),
                json!(["yes", 10.0])
            );
        }
    }

    #[test]
    fn specific_schema_controls_typed_and_inferred_values() {
        for strict_booleans in [false, true] {
            for legacy_octal_numbers in [false, true] {
                let options = options! {
                    schema: serde_saphyr::specific! { strict_booleans: strict_booleans, legacy_octal_numbers: legacy_octal_numbers },
                };
                let boolean = from_str_with_options::<bool>("yes", options.clone());
                let integer = from_str_with_options::<i64>("010", options.clone());
                if strict_booleans {
                    assert!(boolean.is_err());
                } else {
                    assert!(boolean.unwrap());
                }
                if legacy_octal_numbers {
                    assert_eq!(integer.unwrap(), 8);
                } else {
                    assert!(integer.is_err());
                }
                assert_eq!(
                    from_str_with_options::<Value>("[yes, 010]", options).unwrap(),
                    Value::Array(vec![
                        if strict_booleans {
                            json!("yes")
                        } else {
                            json!(true)
                        },
                        if legacy_octal_numbers {
                            json!(8)
                        } else {
                            json!(10.0)
                        },
                    ])
                );
            }
        }
    }

    #[test]
    fn specific_quote_all_has_no_effect_on_deserialization() {
        for strict_booleans in [false, true] {
            for legacy_octal_numbers in [false, true] {
                let options = |quote_all| {
                    options! {
                        schema: serde_saphyr::specific! { strict_booleans: strict_booleans, legacy_octal_numbers: legacy_octal_numbers, quote_all: quote_all },
                    }
                };
                let yaml = "[yes, TrUe, 010, 0x_10, null, 1.5, word]";
                assert_eq!(
                    from_str_with_options::<Value>(yaml, options(false)).unwrap(),
                    from_str_with_options::<Value>(yaml, options(true)).unwrap(),
                );
                assert_eq!(
                    from_str_with_options::<bool>("TrUe", options(false)).unwrap(),
                    from_str_with_options::<bool>("TrUe", options(true)).unwrap(),
                );
                assert_eq!(
                    from_str_with_options::<i64>("0x10", options(false)).unwrap(),
                    from_str_with_options::<i64>("0x10", options(true)).unwrap(),
                );
                assert_eq!(
                    from_str_with_options::<String>("word", options(false)).unwrap(),
                    from_str_with_options::<String>("word", options(true)).unwrap(),
                );
            }
        }
    }

    #[test]
    fn standard_schemas_control_typed_and_inferred_values() {
        for (schema, expected) in [
            (Schema::Yaml11, json!([true, 8, 80])),
            (Schema::Yaml12, json!(["yes", 10, "1:20"])),
            (Schema::Strings, json!(["yes", "010", "1:20"])),
        ] {
            let options = options! { schema: schema };
            assert_eq!(
                from_str_with_options::<Value>("[yes, 010, 1:20]", options).unwrap(),
                expected,
                "{schema:?}"
            );
        }
        assert_eq!(
            from_str_with_options::<i64>("010", options! { schema: Schema::Yaml11 }).unwrap(),
            8
        );
        assert_eq!(
            from_str_with_options::<i64>("010", options! { schema: Schema::Yaml12 }).unwrap(),
            10
        );
        assert_eq!(
            from_str_with_options::<f64>("1:20.5", options! { schema: Schema::Yaml11 }).unwrap(),
            80.5
        );
        assert!(
            from_str_with_options::<f64>("1:20.5", options! { schema: Schema::Yaml12 }).is_err()
        );
        assert!(from_str_with_options::<bool>("yes", options! { schema: Schema::Yaml11 }).unwrap());
        assert!(from_str_with_options::<bool>("yes", options! { schema: Schema::Yaml12 }).is_err());
        assert!(
            from_str_with_options::<bool>("true", options! { schema: Schema::Strings }).is_err()
        );
        assert!(from_str_with_options::<i64>("42", options! { schema: Schema::Strings }).is_err());
        assert!(from_str_with_options::<f64>("1.5", options! { schema: Schema::Strings }).is_err());
        assert_eq!(
            from_str_with_options::<Option<String>>("null", options! { schema: Schema::Strings })
                .unwrap(),
            Some("null".into())
        );
    }

    #[test]
    fn json_schema_rejects_unmatched_plain_scalars() {
        let options = options! { schema: Schema::Json };
        assert_eq!(
            from_str_with_options::<Value>("[true, null, 42, 1.5, \"word\"]", options.clone())
                .unwrap(),
            json!([true, null, 42, 1.5, "word"])
        );
        assert!(from_str_with_options::<bool>("true", options.clone()).unwrap());
        assert_eq!(
            from_str_with_options::<i64>("42", options.clone()).unwrap(),
            42
        );
        assert_eq!(
            from_str_with_options::<f64>("1.5", options.clone()).unwrap(),
            1.5
        );
        for text in ["word", "yes", "TRUE", "010", "0x10", "[true, word]"] {
            assert!(
                from_str_with_options::<Value>(text, options.clone()).is_err(),
                "{text}"
            );
        }
    }

    #[test]
    fn integer_key_equality_uses_the_selected_schema() {
        type Mapping = BTreeMap<String, i32>;
        let octal = serde_saphyr::specific! { strict_booleans: true, legacy_octal_numbers: true };
        assert!(
            from_str_with_options::<Mapping>("010: 1\n8: 2\n", options! { schema: octal }).is_err()
        );
        assert_eq!(
            from_str_with_options::<Mapping>("010: 1\n8: 2\n", options! { schema: Schema::Yaml12 })
                .unwrap()
                .len(),
            2
        );
        assert!(
            from_str_with_options::<Mapping>(
                "010: 1\n10: 2\n",
                options! { schema: Schema::Yaml12 }
            )
            .is_err()
        );
        assert_eq!(
            from_str_with_options::<BTreeMap<String, String>>(
                "0xB: first\n11: second\n",
                options! { schema: Schema::Strings }
            )
            .unwrap()
            .len(),
            2
        );
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    #[allow(deprecated)] // Model a serialized Options value produced before schema existed.
    fn old_serialized_options_default_to_legacy_and_keep_their_flags() {
        let options = options! { strict_booleans: true, legacy_octal_numbers: true };
        let mut json = serde_json::to_value(options).unwrap();
        json.as_object_mut().unwrap().remove("schema");
        let restored: Options = serde_json::from_value(json).unwrap();
        assert_eq!(restored.schema, Schema::Legacy);
        assert_eq!(
            from_str_with_options::<Value>("[yes, 010]", restored).unwrap(),
            json!(["yes", 8])
        );
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn missing_legacy_flags_in_serialized_options_keep_their_defaults() {
        for (present, absent, expected) in [
            (
                "strict_booleans",
                "legacy_octal_numbers",
                json!(["yes", 10.0]),
            ),
            ("legacy_octal_numbers", "strict_booleans", json!([true, 8])),
        ] {
            let mut encoded = serde_json::to_value(Options::default()).unwrap();
            let object = encoded.as_object_mut().unwrap();
            object.remove("schema");
            object.remove(absent);
            object.insert(present.into(), json!(true));
            let restored: Options = serde_json::from_value(encoded).unwrap();
            assert_eq!(restored.schema, Schema::Legacy);
            assert_eq!(
                from_str_with_options::<Value>("[yes, 010]", restored).unwrap(),
                expected
            );
        }
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn explicit_schema_survives_options_serde_roundtrip() {
        for schema in [
            Schema::Strings,
            Schema::Json,
            Schema::Yaml12,
            Schema::Yaml11,
            serde_saphyr::specific! { strict_booleans: true, legacy_octal_numbers: true },
            serde_saphyr::specific! { strict_booleans: true, legacy_octal_numbers: true, quote_all: true },
            serde_saphyr::specific! { strict_booleans: true, legacy_octal_numbers: true, quote_all: true, yaml_12_quoting: true },
        ] {
            let mut encoded = serde_json::to_value(options! { schema: schema }).unwrap();
            let object = encoded.as_object_mut().unwrap();
            object.remove("strict_booleans");
            object.remove("legacy_octal_numbers");
            let restored: Options = serde_json::from_value(encoded).unwrap();
            assert_eq!(restored.schema, schema);
        }
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn older_specific_configurations_default_quote_all_to_false() {
        let schema = serde_saphyr::specific! { strict_booleans: true, legacy_octal_numbers: true };
        let mut encoded = serde_json::to_value(options! { schema: schema }).unwrap();
        encoded["schema"]["specific"]
            .as_object_mut()
            .unwrap()
            .remove("quote_all");
        let restored: Options = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.schema, schema);
        assert_eq!(
            from_str_with_options::<Value>("[yes, 010]", restored).unwrap(),
            json!(["yes", 8])
        );
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn older_specific_configurations_default_yaml_12_quoting_to_false() {
        let schema = serde_saphyr::specific! {
            strict_booleans: true,
            legacy_octal_numbers: true,
            quote_all: true,
        };
        let mut encoded = serde_json::to_value(options! { schema: schema }).unwrap();
        encoded["schema"]["specific"]
            .as_object_mut()
            .unwrap()
            .remove("yaml_12_quoting");
        let restored: Options = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.schema, schema);
        assert_eq!(
            from_str_with_options::<Value>("[yes, 010]", restored).unwrap(),
            json!(["yes", 8])
        );
    }
}

#[cfg(feature = "serialize")]
mod serialize {
    use serde_saphyr::scalar::Schema;
    use serde_saphyr::{
        SerializerOptions, ser_options, to_string_multiple_with_options, to_string_with_options,
    };

    #[test]
    fn defaults_keep_legacy_quoting() {
        let options = SerializerOptions::default();
        assert_eq!(options.schema, Schema::Legacy);
        assert_eq!(
            to_string_with_options(&"yes", options).unwrap(),
            "\"yes\"\n"
        );
    }

    #[test]
    #[allow(deprecated)] // Preserve the old serializer option during migration.
    fn legacy_yaml_12_flag_keeps_quoting_and_directives() {
        let options = ser_options! { yaml_12: true };
        assert_eq!(options.schema, Schema::Legacy);
        assert_eq!(
            to_string_with_options(&"yes", options).unwrap(),
            "%YAML 1.2\n---\nyes\n"
        );
    }

    #[test]
    #[allow(deprecated)] // An explicit schema must take precedence over yaml_12.
    fn explicit_schema_wins_regardless_of_macro_field_order() {
        for options in [
            ser_options! { schema: Schema::Yaml11, yaml_12: true },
            ser_options! { yaml_12: true, schema: Schema::Yaml11 },
        ] {
            let yaml = to_string_with_options(&"yes", options).unwrap();
            assert!(!yaml.contains("%YAML 1.2"), "{yaml}");
            assert!(yaml.contains("\"yes\""), "{yaml}");
        }
        for options in [
            ser_options! { schema: Schema::Yaml12, yaml_12: false },
            ser_options! { yaml_12: false, schema: Schema::Yaml12 },
        ] {
            assert_eq!(
                to_string_with_options(&"yes", options).unwrap(),
                "%YAML 1.2\n---\nyes\n"
            );
        }
    }

    #[test]
    fn specific_deserializer_flags_do_not_change_legacy_string_quoting() {
        for strict_booleans in [false, true] {
            for legacy_octal_numbers in [false, true] {
                let options = ser_options! {
                    schema: serde_saphyr::specific! { strict_booleans: strict_booleans, legacy_octal_numbers: legacy_octal_numbers },
                };
                assert_eq!(
                    to_string_with_options(&"yes", options.clone()).unwrap(),
                    "\"yes\"\n"
                );
                assert_eq!(
                    to_string_with_options(&"0x_10", options.clone()).unwrap(),
                    "\"0x_10\"\n"
                );
                assert_eq!(
                    to_string_with_options(&"true", options).unwrap(),
                    "\"true\"\n"
                );
            }
        }
    }

    #[test]
    fn specific_quote_all_quotes_values_and_disables_automatic_block_styles() {
        for quote_all in [false, true] {
            let options = ser_options! {
                schema: serde_saphyr::specific! { strict_booleans: true, quote_all: quote_all },
                prefer_block_scalars: true,
            };
            assert_eq!(
                to_string_with_options(&"word", options.clone()).unwrap(),
                if quote_all { "'word'\n" } else { "word\n" }
            );
            assert_eq!(
                to_string_with_options(&"yes", options.clone()).unwrap(),
                if quote_all { "'yes'\n" } else { "\"yes\"\n" }
            );
            let multiline =
                to_string_with_options(&"line one\nline two\n", options.clone()).unwrap();
            if quote_all {
                assert_eq!(multiline, "\"line one\\nline two\\n\"\n");
            } else {
                assert!(multiline.starts_with("|\n"), "{multiline}");
            }

            // quote_all concerns ordinary values; explicit wrappers keep their style.
            let literal =
                to_string_with_options(&serde_saphyr::LitStr("line one\nline two\n"), options)
                    .unwrap();
            assert!(literal.starts_with("|\n"), "{literal}");
        }
    }

    #[test]
    #[allow(deprecated)] // Explicit Specific settings override the old field in either direction.
    fn specific_quote_all_takes_precedence_over_the_legacy_field() {
        for quote_all in [false, true] {
            let schema = serde_saphyr::specific! { strict_booleans: true, quote_all: quote_all };
            for options in [
                ser_options! { schema: schema, quote_all: !quote_all },
                ser_options! { quote_all: !quote_all, schema: schema },
            ] {
                assert_eq!(
                    to_string_with_options(&"word", options.clone()).unwrap(),
                    if quote_all { "'word'\n" } else { "word\n" }
                );
                let mapping = std::collections::BTreeMap::from([("word", "word")]);
                assert_eq!(
                    to_string_with_options(&mapping, options).unwrap(),
                    if quote_all {
                        "word: 'word'\n"
                    } else {
                        "word: word\n"
                    }
                );
            }
        }
    }

    #[test]
    #[allow(deprecated)] // Standard schemas override the old presentation flag.
    fn strings_and_json_schemas_ignore_legacy_quote_all() {
        for schema in [Schema::Strings, Schema::Json] {
            for text in ["word", "true", "42", "null"] {
                let actual =
                    to_string_with_options(&text, ser_options! { schema: schema, quote_all: true })
                        .unwrap();
                assert_eq!(
                    actual,
                    if schema == Schema::Strings {
                        format!("{text}\n")
                    } else {
                        format!("\"{text}\"\n")
                    }
                );
            }
            for text in ["", "needs: quoting", "# comment", " leading space"] {
                let actual =
                    to_string_with_options(&text, ser_options! { schema: schema, quote_all: true })
                        .unwrap();
                assert_eq!(
                    actual,
                    to_string_with_options(&text, ser_options! { schema: schema }).unwrap()
                );
                assert_ne!(actual, format!("{text}\n"));
            }
            let multiline = to_string_with_options(
                &"line one\nline two\n",
                ser_options! {
                    schema: schema,
                    quote_all: true,
                    prefer_block_scalars: true,
                },
            )
            .unwrap();
            assert!(multiline.starts_with("|\n"), "{schema:?}: {multiline}");
        }
    }

    #[test]
    fn json_schema_quotes_string_keys_without_changing_typed_values() {
        #[derive(serde::Serialize)]
        struct Scalars {
            word: &'static str,
            enabled: bool,
            number: u32,
        }
        let yaml = to_string_with_options(
            &Scalars {
                word: "hello",
                enabled: true,
                number: 42,
            },
            ser_options! { schema: Schema::Json },
        )
        .unwrap();
        assert_eq!(
            yaml,
            "\"word\": \"hello\"\n\"enabled\": true\n\"number\": 42\n"
        );
        #[cfg(feature = "deserialize")]
        assert_eq!(
            serde_saphyr::from_str_with_options::<serde_json::Value>(
                &yaml,
                serde_saphyr::options! { schema: Schema::Json },
            )
            .unwrap(),
            serde_json::json!({ "word": "hello", "enabled": true, "number": 42 })
        );
    }

    #[test]
    #[allow(deprecated)] // Preserve the combined legacy serializer flags during migration.
    fn legacy_quote_all_and_yaml_12_keep_their_combined_behavior() {
        for yaml_12 in [false, true] {
            for quote_all in [false, true] {
                let options = ser_options! { yaml_12: yaml_12, quote_all: quote_all };
                assert_eq!(options.schema, Schema::Legacy);
                let expected_value = if quote_all {
                    "'yes'\n"
                } else if yaml_12 {
                    "yes\n"
                } else {
                    "\"yes\"\n"
                };
                assert_eq!(
                    to_string_with_options(&"yes", options).unwrap(),
                    if yaml_12 {
                        format!("%YAML 1.2\n---\n{expected_value}")
                    } else {
                        expected_value.to_owned()
                    }
                );
            }
        }
    }

    #[test]
    fn json_and_strings_schemas_control_string_quoting() {
        for text in ["word", "true", "42"] {
            assert_eq!(
                to_string_with_options(&text, ser_options! { schema: Schema::Json }).unwrap(),
                format!("\"{text}\"\n")
            );
            assert_eq!(
                to_string_with_options(&text, ser_options! { schema: Schema::Strings }).unwrap(),
                format!("{text}\n")
            );
        }
    }

    #[test]
    fn explicit_yaml12_controls_directives_and_multi_document_boundaries() {
        assert_eq!(
            to_string_multiple_with_options(
                &["yes", "on"],
                ser_options! { schema: Schema::Yaml12 }
            )
            .unwrap(),
            "%YAML 1.2\n---\nyes\n...\n%YAML 1.2\n---\non\n"
        );
        assert_eq!(
            to_string_multiple_with_options(
                &["yes", "on"],
                ser_options! { schema: Schema::Yaml12, no_lang_directive: true }
            )
            .unwrap(),
            "yes\n---\non\n"
        );
        assert_eq!(
            to_string_with_options(
                &"yes",
                ser_options! { schema: Schema::Yaml12, no_lang_directive: true }
            )
            .unwrap(),
            "yes\n"
        );
    }
}
