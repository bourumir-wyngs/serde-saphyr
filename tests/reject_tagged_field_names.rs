#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::{
    DuplicateKeyPolicy, Error, Options, Tagged, from_reader_with_options, from_str,
    from_str_with_options, options,
};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct Entry {
    name: Tagged<String>,
    count: Option<u32>,
}

fn strict_options() -> Options {
    options! { reject_tagged_field_names: true }
}

fn inner_error(mut error: &Error) -> &Error {
    loop {
        match error {
            Error::WithSnippet { error: inner, .. } | Error::AliasError { error: inner, .. } => {
                error = inner;
            }
            _ => return error,
        }
    }
}

fn assert_rejected<T: for<'de> Deserialize<'de> + std::fmt::Debug>(yaml: &str) {
    let error = from_str_with_options::<T>(yaml, strict_options()).unwrap_err();
    assert!(
        matches!(inner_error(&error), Error::TaggedStructFieldName { .. }),
        "{yaml:?}: unexpected error: {error:?}"
    );
    assert!(error.location().is_some());
}

#[test]
fn tagged_field_names_keep_their_existing_behavior_by_default() {
    assert!(!Options::default().reject_tagged_field_names);
    let entry: Entry = from_str("{name: a, !custom count: 1}").unwrap();
    assert_eq!(entry.count, Some(1));
    assert_eq!(entry.name.0, "a");
}

#[test]
fn string_tags_and_renamed_fields_are_allowed() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Renamed {
        #[serde(rename = "0.0")]
        name: String,
    }

    for yaml in [
        "0.0: Alice",
        "'0.0': Alice",
        "!!str 0.0: Alice",
        "! 0.0: Alice",
        "!<tag:yaml.org,2002:str> 0.0: Alice",
        "%TAG !e! tag:yaml.org,2002:\n---\n!e!str 0.0: Alice",
    ] {
        let value: Renamed = from_str_with_options(yaml, strict_options()).unwrap();
        assert_eq!(value.name, "Alice", "{yaml:?}");
    }
}

#[test]
fn other_explicit_scalar_tags_are_rejected_on_field_names() {
    for yaml in [
        "{name: a, !custom count: 1}",
        "{name: a, !str count: 1}",
        "{name: a, !<tag:yaml.org,2002:!str> count: 1}",
        "{name: a, !<tag:example.com,2026:field> count: 1}",
        "%TAG !e! tag:example.com,2026:\n---\n{name: a, !e!field count: 1}",
        "%TAG !! tag:example.com,2026:\n---\n{name: a, !!str count: 1}",
        "{name: a, !!null count: 1}",
        "{name: a, !!binary Y291bnQ=: 1}",
        "{name: a, !!value count: 1}",
        "{name: a, ? !custom count : 1}",
        "name: a\n!custom count: 1\n",
        "{name: a, !custom unknown: 1}",
    ] {
        assert_rejected::<Entry>(yaml);
    }
}

#[test]
fn field_tag_errors_preserve_source_spelling_through_replay_and_reader_input() {
    for (yaml, expected, policy) in [
        (
            "{name: a, !!int count: 1}",
            "!!int",
            DuplicateKeyPolicy::Error,
        ),
        (
            "%TAG !e! tag:example.com,2026:\n---\n{name: a, !e!secret count: 1}",
            "!e!secret",
            DuplicateKeyPolicy::Error,
        ),
        (
            "{name: a, !<tag:example.com,2026:field> count: 1}",
            "!<tag:example.com,2026:field>",
            DuplicateKeyPolicy::Error,
        ),
        (
            "%TAG !e! tag:example.com,2026:\n---\n{name: &key !e!secret count, *key : 1}",
            "!e!secret",
            DuplicateKeyPolicy::Error,
        ),
        (
            "%TAG !e! tag:example.com,2026:\n---\n{name: a, <<: {!e!secret count: 1}}",
            "!e!secret",
            DuplicateKeyPolicy::Error,
        ),
        (
            "{name: a, !<tag:yaml.org,2002:!str> count: 1, count: 2}",
            "!<tag:yaml.org,2002:!str>",
            DuplicateKeyPolicy::LastWins,
        ),
    ] {
        let error = from_str_with_options::<Entry>(
            yaml,
            options! { reject_tagged_field_names: true, duplicate_keys: policy },
        )
        .unwrap_err();
        let reader_error = from_reader_with_options::<_, Entry>(
            yaml.as_bytes(),
            options! { reject_tagged_field_names: true, duplicate_keys: policy },
        )
        .unwrap_err();
        for error in [&error, &reader_error] {
            assert!(
                matches!(inner_error(error), Error::TaggedStructFieldName { tag, .. } if tag == expected),
                "{policy:?}, {yaml:?}: unexpected error: {error:?}"
            );
            assert!(error.location().is_some());
        }
    }
}

#[test]
fn ignored_unknown_field_names_are_still_checked_for_misplaced_tags() {
    let entry: Entry =
        from_str_with_options("{name: a, count: 1, unknown: 2}", strict_options()).unwrap();
    assert_eq!(entry.name.0, "a");
    assert_eq!(entry.count, Some(1));

    assert_rejected::<Entry>("{name: a, count: 1, !secret unknown: 2}");
}

#[test]
fn tagged_values_still_reach_the_application() {
    let entry: Entry =
        from_str_with_options("{name: !secret API_TOKEN, count: 1}", strict_options()).unwrap();
    assert_eq!(entry.name.0, "API_TOKEN");
    assert_eq!(entry.name.1.as_deref(), Some("!secret"));
    assert_eq!(entry.count, Some(1));
}

#[test]
fn aliases_and_merged_field_names_are_checked() {
    assert_rejected::<Entry>("{name: &key !custom count, *key : 1}");
    assert_rejected::<Entry>("{name: a, <<: {!custom count: 1}}");
    assert_rejected::<Entry>("{name: a, !!merge <<: {!!value count: 1}}");

    let entry: Entry =
        from_str_with_options("{name: a, !!merge <<: {count: 1}}", strict_options()).unwrap();
    assert_eq!(entry.count, Some(1));
}

#[test]
fn buffered_duplicate_policy_checks_field_names() {
    let options = options! {
        reject_tagged_field_names: true,
        duplicate_keys: DuplicateKeyPolicy::LastWins,
    };
    let error = from_str_with_options::<Entry>("{name: a, !custom count: 1}", options).unwrap_err();
    assert!(matches!(
        inner_error(&error),
        Error::TaggedStructFieldName { .. }
    ));

    let entry: Entry = from_str_with_options(
        "{name: a, !!str count: 1, !!str count: 2}",
        options! {
            reject_tagged_field_names: true,
            duplicate_keys: DuplicateKeyPolicy::LastWins,
        },
    )
    .unwrap();
    assert_eq!(entry.count, Some(2));
}

fn assert_rejected_with_duplicate_policy(yaml: &str, duplicate_keys: DuplicateKeyPolicy) {
    let error = from_str_with_options::<Entry>(
        yaml,
        options! { reject_tagged_field_names: true, duplicate_keys: duplicate_keys },
    )
    .unwrap_err();
    assert!(
        matches!(inner_error(&error), Error::TaggedStructFieldName { tag, .. } if tag == "!str"),
        "{duplicate_keys:?}, {yaml:?}: unexpected error: {error:?}"
    );
    assert!(error.location().is_some());
}

#[test]
fn forbidden_tags_are_checked_before_duplicate_fields_are_discarded() {
    for (policy, expected) in [
        (DuplicateKeyPolicy::FirstWins, 1),
        (DuplicateKeyPolicy::LastWins, 2),
    ] {
        for yaml in [
            "{name: a, count: 1, !str count: 2}",
            "{name: a, !str count: 1, count: 2}",
            "{name: &key !str count, count: 1, *key : 2}",
            "{name: &key !str count, *key : 1, count: 2}",
        ] {
            assert_rejected_with_duplicate_policy(yaml, policy);
        }

        let entry: Entry = from_str_with_options(
            "{name: a, !!str count: 1, count: 2}",
            options! { reject_tagged_field_names: true, duplicate_keys: policy },
        )
        .unwrap();
        assert_eq!(entry.count, Some(expected));
    }
}

#[test]
fn forbidden_tags_are_checked_before_merge_precedence_discards_fields() {
    for policy in [DuplicateKeyPolicy::FirstWins, DuplicateKeyPolicy::LastWins] {
        for yaml in [
            "{name: a, count: 9, <<: {!str count: 1}}",
            "{name: a, <<: {count: 1, !str count: 2}}",
            "{name: a, <<: {!str count: 1, count: 2}}",
            "{name: a, <<: [{count: 1}, {!str count: 2}]}",
            "{name: a, <<: {count: 1, <<: {!str count: 2}}}",
            "{name: &key !str count, count: 9, <<: {*key : 1}}",
        ] {
            assert_rejected_with_duplicate_policy(yaml, policy);
        }
    }
    assert_rejected_with_duplicate_policy(
        "{name: a, count: 9, <<: {!str count: 1}}",
        DuplicateKeyPolicy::Error,
    );
}

#[test]
fn duplicate_and_merged_map_keys_keep_their_existing_behavior() {
    #[derive(Debug, Deserialize)]
    struct WithMap {
        values: BTreeMap<String, u32>,
    }

    for (policy, expected) in [
        (DuplicateKeyPolicy::FirstWins, 1),
        (DuplicateKeyPolicy::LastWins, 2),
    ] {
        let map: BTreeMap<String, u32> = from_str_with_options(
            "{count: 1, !str count: 2}",
            options! { reject_tagged_field_names: true, duplicate_keys: policy },
        )
        .unwrap();
        assert_eq!(map.get("count"), Some(&expected));

        for (yaml, winner) in [
            ("{values: {count: 1, !str count: 2}}", expected),
            ("{values: {count: 9, <<: {!str count: 1}}}", 9),
        ] {
            let value: WithMap = from_str_with_options(
                yaml,
                options! { reject_tagged_field_names: true, duplicate_keys: policy },
            )
            .unwrap();
            assert_eq!(
                value.values.get("count"),
                Some(&winner),
                "{policy:?}, {yaml:?}"
            );
        }
    }
}

#[test]
fn struct_values_inside_maps_still_validate_field_tags() {
    for policy in [DuplicateKeyPolicy::FirstWins, DuplicateKeyPolicy::LastWins] {
        for error in [
            from_str_with_options::<BTreeMap<String, Entry>>(
                "{item: {name: a, !str count: 1}}",
                options! { reject_tagged_field_names: true, duplicate_keys: policy },
            )
            .unwrap_err(),
            from_str_with_options::<BTreeMap<usize, Entry>>(
                "{7: {name: a, !str count: 1}}",
                options! { reject_tagged_field_names: true, duplicate_keys: policy },
            )
            .unwrap_err(),
        ] {
            assert!(
                matches!(inner_error(&error), Error::TaggedStructFieldName { tag, .. } if tag == "!str"),
                "{policy:?}: unexpected error: {error:?}"
            );
        }
    }
}

#[derive(Debug, Deserialize)]
struct Outer {
    #[allow(dead_code)]
    entry: Entry,
}

#[derive(Debug, Deserialize)]
enum External {
    Entry {
        #[allow(dead_code)]
        count: u32,
    },
}

#[test]
fn nested_structs_and_external_struct_variants_are_checked() {
    assert_rejected::<Outer>("{entry: {name: a, !custom count: 1}}");
    assert_rejected::<Outer>("{!custom entry: {name: a}}");
    assert_rejected::<External>("!Entry {!custom count: 1}");
    assert_rejected::<External>("{Entry: {!custom count: 1}}");
}

#[test]
fn handwritten_struct_visitors_with_string_keys_are_checked() {
    #[derive(Debug)]
    struct StringKeys;

    impl<'de> Deserialize<'de> for StringKeys {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct StructVisitor;

            impl<'de> serde::de::Visitor<'de> for StructVisitor {
                type Value = StringKeys;

                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("a struct with string field names")
                }

                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Self::Value, A::Error> {
                    while let Some(key) = map.next_key::<String>()? {
                        if key != "count" {
                            return Err(serde::de::Error::unknown_field(&key, &["count"]));
                        }
                        let _: u32 = map.next_value()?;
                    }
                    Ok(StringKeys)
                }
            }

            deserializer.deserialize_struct("StringKeys", &["count"], StructVisitor)
        }
    }

    assert_rejected::<StringKeys>("!custom count: 1");
    from_str_with_options::<StringKeys>("!!str count: 1", strict_options()).unwrap();
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
enum MapKey {
    Port(u16),
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(variant_identifier)]
enum NamedKey {
    Count,
}

#[test]
fn string_integer_and_enum_mapping_keys_are_unchanged() {
    let strings: BTreeMap<String, u32> =
        from_str_with_options("{!custom count: 1}", strict_options()).unwrap();
    assert_eq!(strings.get("count"), Some(&1));

    let integers: BTreeMap<usize, usize> =
        from_str_with_options("{!!int 7: 9}", strict_options()).unwrap();
    assert_eq!(integers.get(&7), Some(&9));

    let enums: BTreeMap<MapKey, u32> =
        from_str_with_options("{!Port 8080: 1}", strict_options()).unwrap();
    assert_eq!(enums.get(&MapKey::Port(8080)), Some(&1));

    let identifiers: BTreeMap<NamedKey, u32> =
        from_str_with_options("{!custom Count: 1}", strict_options()).unwrap();
    assert_eq!(identifiers.get(&NamedKey::Count), Some(&1));
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum Internal {
    Entry { count: u32 },
}

#[test]
fn identifier_values_are_not_field_names() {
    #[derive(Debug, Deserialize)]
    struct WithKind {
        kind: NamedKey,
    }

    let value: WithKind = from_str_with_options("{kind: !custom Count}", strict_options()).unwrap();
    assert_eq!(value.kind, NamedKey::Count);

    let internal: Internal =
        from_str_with_options("{kind: !!str Entry, count: 1}", strict_options()).unwrap();
    assert_eq!(internal, Internal::Entry { count: 1 });
}

#[test]
fn serde_buffered_and_flattened_structs_are_outside_the_policy() {
    #[derive(Debug, Deserialize)]
    struct Plain {
        count: u32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    enum Untagged {
        Entry(Plain),
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "kind", content = "data")]
    enum Adjacent {
        Entry(Plain),
    }

    #[derive(Debug, Deserialize)]
    struct Flattened {
        #[serde(flatten)]
        entry: Plain,
    }

    // The YAML 1.1 value tag stays a scalar when Serde buffers it. A simple
    // application tag can instead become a synthetic enum mapping on this path.
    let internal: Internal =
        from_str_with_options("{kind: Entry, !!value count: 1}", strict_options()).unwrap();
    assert_eq!(internal, Internal::Entry { count: 1 });

    let Untagged::Entry(plain) =
        from_str_with_options("{!!value count: 1}", strict_options()).unwrap();
    assert_eq!(plain.count, 1);

    let Adjacent::Entry(plain) =
        from_str_with_options("{data: {!!value count: 1}, kind: Entry}", strict_options()).unwrap();
    assert_eq!(plain.count, 1);

    let flattened: Flattened =
        from_str_with_options("{!!value count: 1}", strict_options()).unwrap();
    assert_eq!(flattened.entry.count, 1);
}

#[test]
fn error_keeps_the_tag_and_escapes_control_characters_in_diagnostics() {
    let error =
        from_str_with_options::<Entry>("{name: a, !evil%0Aforged count: 1}", strict_options())
            .unwrap_err();
    let inner = inner_error(&error);
    assert!(
        matches!(inner, Error::TaggedStructFieldName { tag, .. } if tag == "!evil\nforged"),
        "{inner:?}"
    );
    assert!(inner.location().is_some());
    let message = inner.to_string();
    assert!(message.contains(r"!evil\nforged"), "{message:?}");
    assert!(!message.chars().any(char::is_control), "{message:?}");
}
