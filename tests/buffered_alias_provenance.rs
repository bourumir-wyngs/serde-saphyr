#![cfg(feature = "deserialize")]

//! Buffering for duplicate keys and inline merges must not invent alias errors.

use std::collections::BTreeMap;
use std::fmt::Debug;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_saphyr::{DuplicateKeyPolicy, Error, Location, Spanned};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Settings<T> {
    settings: T,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Port {
    port: u16,
}

fn parse_both<T: DeserializeOwned>(
    yaml: &str,
    policy: DuplicateKeyPolicy,
) -> [Result<T, Error>; 2] {
    let options = serde_saphyr::options! { duplicate_keys: policy };
    [
        serde_saphyr::from_str_with_options(yaml, options.clone()),
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options),
    ]
}

fn coordinates(location: Location) -> (u64, u64) {
    (location.line(), location.column())
}

fn position(yaml: &str, token: &str) -> (u64, u64) {
    let offset = yaml.find(token).expect("fixture must contain the token");
    let prefix = &yaml[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap().chars().count() + 1;
    (line as u64, column as u64)
}

fn assert_plain_error<T: DeserializeOwned + Debug>(yaml: &str, policy: DuplicateKeyPolicy) {
    for outer in parse_both::<T>(yaml, policy) {
        let outer = outer.unwrap_err();
        let error = outer.without_snippet();
        assert!(
            matches!(error, Error::InvalidScalar { ty: "u16", .. }),
            "an ordinary buffered value must expose the original error: {error:?}"
        );
        assert_eq!(
            coordinates(error.location().unwrap()),
            position(yaml, "nope")
        );
        let locations = error.locations().unwrap();
        assert_eq!(locations.reference_location, locations.defined_location);
        let rendered = outer.to_string();
        assert!(!rendered.contains("defined here"), "{rendered}");
        assert!(!rendered.contains("used here"), "{rendered}");
    }
}

fn assert_alias_error<T: DeserializeOwned + Debug>(
    yaml: &str,
    policy: DuplicateKeyPolicy,
    definition: &str,
) {
    for outer in parse_both::<T>(yaml, policy) {
        let outer = outer.unwrap_err();
        let Error::AliasError {
            error, locations, ..
        } = outer.without_snippet()
        else {
            panic!("a genuine nested alias must retain its provenance: {outer:?}");
        };
        assert!(
            matches!(error.as_ref(), Error::InvalidScalar { ty: "u16", .. }),
            "buffering must not add redundant alias wrappers: {error:?}"
        );
        assert_eq!(
            coordinates(locations.reference_location),
            position(yaml, "*b")
        );
        assert_eq!(
            coordinates(locations.defined_location),
            position(yaml, definition)
        );
        assert_eq!(
            coordinates(error.location().unwrap()),
            position(yaml, "nope")
        );
    }
}

#[test]
fn last_wins_nested_struct_does_not_invent_aliases() {
    for yaml in [
        "settings:\n  port: nope\n",
        "settings: {port: 80}\nsettings:\n  port: nope\n",
        "settings:\n  port: 80\n  port: nope\n",
    ] {
        assert_plain_error::<Settings<Port>>(yaml, DuplicateKeyPolicy::LastWins);
    }
}

#[test]
fn last_wins_nested_maps_and_sequences_do_not_invent_aliases() {
    assert_plain_error::<BTreeMap<String, BTreeMap<String, u16>>>(
        "settings:\n  port: nope\n",
        DuplicateKeyPolicy::LastWins,
    );
    assert_plain_error::<BTreeMap<u32, BTreeMap<u32, u16>>>(
        "1:\n  2: nope\n",
        DuplicateKeyPolicy::LastWins,
    );
    assert_plain_error::<Settings<Vec<Vec<Port>>>>(
        "settings:\n  - - port: nope\n",
        DuplicateKeyPolicy::LastWins,
    );
}

#[test]
fn inline_merge_values_do_not_invent_aliases() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        assert_plain_error::<Port>("<<: {port: nope}\n", policy);
        assert_plain_error::<Settings<Port>>("<<: {settings: {port: nope}}\n", policy);
        assert_plain_error::<Settings<Vec<Port>>>("<<: {settings: [{port: nope}]}\n", policy);
    }
}

#[test]
fn inline_merge_sequences_do_not_invent_aliases() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        assert_plain_error::<Port>("<<: [{port: nope}, {ignored: 1}]\n", policy);
        assert_plain_error::<Settings<Port>>(
            "<<: [{settings: {port: nope}}, {ignored: 1}]\n",
            policy,
        );
    }
}

#[test]
fn last_wins_keeps_genuine_scalar_aliases_inside_buffered_values() {
    for yaml in [
        "base: &b nope\nsettings:\n  port: *b\n",
        "base: &b nope\nsettings: {port: 80}\nsettings:\n  port: *b\n",
    ] {
        assert_alias_error::<Settings<Port>>(yaml, DuplicateKeyPolicy::LastWins, "nope");
    }
    assert_alias_error::<Settings<Vec<Port>>>(
        "base: &b nope\nsettings:\n  - port: *b\n",
        DuplicateKeyPolicy::LastWins,
        "nope",
    );
}

#[test]
fn last_wins_keeps_genuine_mapping_aliases_inside_buffered_values() {
    assert_alias_error::<Settings<Vec<Port>>>(
        "base: &b\n  port: nope\nsettings:\n  - *b\n",
        DuplicateKeyPolicy::LastWins,
        "port:",
    );
    assert_alias_error::<Settings<BTreeMap<String, Port>>>(
        "base: &b\n  port: nope\nsettings:\n  copy: *b\n",
        DuplicateKeyPolicy::LastWins,
        "port:",
    );
}

#[test]
fn inline_merges_keep_genuine_nested_aliases() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        assert_alias_error::<Settings<Port>>(
            "base: &b nope\n<<: {settings: {port: *b}}\n",
            policy,
            "nope",
        );
        assert_alias_error::<Settings<Vec<Port>>>(
            "base: &b\n  port: nope\n<<: [{settings: [*b]}]\n",
            policy,
            "port:",
        );
    }
}

#[test]
fn aliased_merge_sources_keep_their_alias_errors() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        for yaml in [
            "base: &b {port: nope}\n<<: *b\n",
            "base: &b {port: nope}\n<<: [*b]\n",
            "base: &b {port: nope}\n<<: {<<: *b}\n",
        ] {
            assert_alias_error::<Port>(yaml, policy, "nope");
        }
    }
}

#[derive(Debug, Deserialize)]
struct SpannedSettings {
    port: Spanned<u16>,
    numbers: Vec<Spanned<u16>>,
}

#[test]
fn last_wins_keeps_each_ordinary_spanned_value_at_its_own_location() {
    for yaml in [
        "settings:\n  port: 7\n  numbers: [8, 9]\n",
        "settings: {port: 0, numbers: []}\nsettings:\n  port: 7\n  numbers: [8, 9]\n",
    ] {
        for parsed in parse_both::<Settings<SpannedSettings>>(yaml, DuplicateKeyPolicy::LastWins) {
            let settings = parsed.unwrap().settings;
            assert_eq!(settings.numbers.len(), 2);
            for (value, expected, token) in [
                (&settings.port, 7, "7"),
                (&settings.numbers[0], 8, "8"),
                (&settings.numbers[1], 9, "9"),
            ] {
                assert_eq!(value.value, expected);
                assert_eq!(value.referenced, value.defined);
                assert_eq!(coordinates(value.defined), position(yaml, token));
            }
        }
    }
}

#[test]
fn last_wins_preserves_individual_nested_scalar_alias_spans() {
    let yaml = "base: &b 7\nsettings:\n  port: *b\n  numbers: [*b, 9]\n";
    for parsed in parse_both::<Settings<SpannedSettings>>(yaml, DuplicateKeyPolicy::LastWins) {
        let settings = parsed.unwrap().settings;
        assert_eq!(settings.port.value, 7);
        assert_eq!(settings.numbers[0].value, 7);
        assert_eq!(coordinates(settings.port.referenced), (3, 9));
        assert_eq!(coordinates(settings.numbers[0].referenced), (4, 13));
        assert_eq!(coordinates(settings.port.defined), position(yaml, "7"));
        assert_eq!(settings.port.defined, settings.numbers[0].defined);
        assert_eq!(settings.numbers[1].referenced, settings.numbers[1].defined);
        assert_eq!(
            coordinates(settings.numbers[1].defined),
            position(yaml, "9")
        );
    }
}

#[test]
fn last_wins_preserves_mapping_alias_spans_for_nested_values() {
    let yaml = "base: &b\n  port: 7\n  numbers: [8, 9]\nsettings: *b\n";
    for parsed in parse_both::<Settings<SpannedSettings>>(yaml, DuplicateKeyPolicy::LastWins) {
        let settings = parsed.unwrap().settings;
        for (value, expected, token) in [
            (&settings.port, 7, "7"),
            (&settings.numbers[0], 8, "8"),
            (&settings.numbers[1], 9, "9"),
        ] {
            assert_eq!(value.value, expected);
            assert_eq!(coordinates(value.referenced), position(yaml, "*b"));
            assert_eq!(coordinates(value.defined), position(yaml, token));
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum TaggedPort {
    Port(u16),
    Pair(u16, u16),
    Map { port: u16 },
}

#[test]
fn tagged_enum_buffering_does_not_invent_aliases() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        for yaml in [
            "settings: !Port nope\n",
            "settings: !Pair [nope, 9]\n",
            "settings: !Map {port: nope}\n",
        ] {
            assert_plain_error::<Settings<TaggedPort>>(yaml, policy);
        }
    }
}

#[test]
fn tagged_enum_buffering_preserves_nested_alias_errors() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        for yaml in [
            "base: &b !Port nope\nsettings: *b\n",
            "base: &b nope\nsettings: !Pair [*b, 9]\n",
            "base: &b nope\nsettings: !Map {port: *b}\n",
        ] {
            assert_alias_error::<Settings<TaggedPort>>(yaml, policy, "nope");
        }
    }
}

#[derive(Debug, Deserialize)]
enum TaggedSpans {
    Port(Spanned<u16>),
    Pair(Spanned<u16>, Spanned<u16>),
}

#[test]
fn tagged_scalar_replay_preserves_the_alias_span() {
    let yaml = "base: &b !Port 7\nsettings: *b\n";
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        for parsed in parse_both::<Settings<TaggedSpans>>(yaml, policy) {
            let TaggedSpans::Port(port) = parsed.unwrap().settings else {
                panic!("expected the tag-selected newtype variant");
            };
            assert_eq!(port.value, 7);
            assert_eq!(coordinates(port.referenced), position(yaml, "*b"));
            assert_eq!(coordinates(port.defined), position(yaml, "7"));
        }
    }
}

#[test]
fn tagged_tuple_replay_preserves_both_nested_and_enclosing_alias_spans() {
    for (yaml, second_is_aliased) in [
        ("base: &b 7\nsettings: !Pair [*b, 9]\n", false),
        ("base: &b !Pair [7, 9]\nsettings: *b\n", true),
    ] {
        for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
            for parsed in parse_both::<Settings<TaggedSpans>>(yaml, policy) {
                let TaggedSpans::Pair(first, second) = parsed.unwrap().settings else {
                    panic!("expected the tag-selected tuple variant");
                };
                assert_eq!(first.value, 7);
                assert_eq!(second.value, 9);
                assert_eq!(coordinates(first.referenced), position(yaml, "*b"));
                assert_eq!(coordinates(first.defined), position(yaml, "7"));
                assert_eq!(coordinates(second.defined), position(yaml, "9"));
                if second_is_aliased {
                    assert_eq!(coordinates(second.referenced), position(yaml, "*b"));
                } else {
                    assert_eq!(second.referenced, second.defined);
                }
            }
        }
    }
}

#[test]
fn inline_merge_spans_keep_the_merge_reference_location() {
    #[derive(Debug, Deserialize)]
    struct SpannedPort {
        port: Spanned<u16>,
    }

    for yaml in ["<<: {port: 7}\n", "<<: [{port: 7}]\n"] {
        for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
            for parsed in parse_both::<SpannedPort>(yaml, policy) {
                let port = parsed.unwrap().port;
                assert_eq!(port.value, 7);
                assert_eq!(coordinates(port.referenced), position(yaml, "{port"));
                assert_eq!(coordinates(port.defined), position(yaml, "7"));
                assert_ne!(port.referenced, port.defined);
            }
        }
    }
}

#[cfg(feature = "garde")]
#[test]
fn last_wins_validation_uses_the_plain_leaf_location_without_alias_labels() {
    #[derive(Debug, Deserialize, garde::Validate)]
    struct Document {
        #[garde(dive)]
        settings: Name,
    }

    #[derive(Debug, Deserialize, garde::Validate)]
    struct Name {
        #[garde(length(min = 2))]
        name: String,
    }

    let yaml = "settings:\n  name: x\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    for error in [
        serde_saphyr::from_str_with_options_valid::<Document>(yaml, options.clone()).unwrap_err(),
        serde_saphyr::from_reader_with_options_valid::<_, Document>(yaml.as_bytes(), options)
            .unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::ValidationError { .. }
        ));
        let locations = error.locations().expect("nested validation location");
        assert_eq!(locations.reference_location, locations.defined_location);
        assert_eq!(coordinates(locations.defined_location), position(yaml, "x"));
        let rendered = error.to_string();
        assert!(rendered.contains("for `settings.name`"), "{rendered}");
        assert!(!rendered.contains("the value is used here"), "{rendered}");
        assert!(!rendered.contains("from the anchor"), "{rendered}");
    }
}

#[cfg(feature = "include")]
fn include_options(child: &'static str) -> serde_saphyr::Options {
    serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins }.with_include_resolver(
        move |request: serde_saphyr::IncludeRequest| {
            assert_eq!(request.spec, "child.yaml");
            Ok(serde_saphyr::ResolvedInclude::new(
                "child.yaml",
                "child.yaml",
                serde_saphyr::InputSource::from_string(child.to_owned()),
            ))
        },
    )
}

#[cfg(feature = "include")]
#[test]
fn last_wins_preserves_included_plain_and_alias_source_identity() {
    #[derive(Debug, Deserialize)]
    struct Document {
        top: Spanned<u16>,
        settings: SpannedSettings,
    }

    let yaml = "top: 1\nsettings: !include child.yaml\n";
    let child = "base: &b 7\nport: *b\nnumbers: [8, 9]\n";
    let options = include_options(child);
    for parsed in [
        serde_saphyr::from_str_with_options::<Document>(yaml, options.clone()).unwrap(),
        serde_saphyr::from_reader_with_options::<_, Document>(yaml.as_bytes(), options).unwrap(),
    ] {
        assert_eq!(parsed.top.referenced, parsed.top.defined);
        let settings = parsed.settings;
        let child_id = settings.port.defined.source_id();
        assert_ne!(child_id, parsed.top.defined.source_id());
        assert_eq!(settings.port.referenced.source_id(), child_id);
        assert_eq!(coordinates(settings.port.referenced), position(child, "*b"));
        assert_eq!(coordinates(settings.port.defined), position(child, "7"));
        for (number, token) in settings.numbers.iter().zip(["8", "9"]) {
            assert_eq!(number.defined.source_id(), child_id);
            assert_eq!(number.referenced, number.defined);
            assert_eq!(coordinates(number.defined), position(child, token));
        }
    }
}

#[cfg(feature = "include")]
#[test]
fn last_wins_preserves_included_error_source_identity() {
    let yaml = "settings: !include child.yaml\n";
    for (child, is_alias) in [("port: nope\n", false), ("base: &b nope\nport: *b\n", true)] {
        let options = include_options(child);
        for outer in [
            serde_saphyr::from_str_with_options::<Settings<Port>>(yaml, options.clone())
                .unwrap_err(),
            serde_saphyr::from_reader_with_options::<_, Settings<Port>>(yaml.as_bytes(), options)
                .unwrap_err(),
        ] {
            let error = outer.without_snippet();
            let locations = error.locations().unwrap();
            assert_ne!(locations.defined_location.source_id(), 0);
            assert_eq!(
                locations.reference_location.source_id(),
                locations.defined_location.source_id()
            );
            assert_eq!(
                coordinates(locations.defined_location),
                position(child, "nope")
            );
            if is_alias {
                assert!(matches!(error, Error::AliasError { .. }), "{error:?}");
                assert_eq!(
                    coordinates(locations.reference_location),
                    position(child, "*b")
                );
            } else {
                assert!(
                    matches!(error, Error::InvalidScalar { ty: "u16", .. }),
                    "{error:?}"
                );
                assert_eq!(locations.reference_location, locations.defined_location);
            }
            assert!(outer.to_string().contains("child.yaml"), "{outer}");
        }
    }
}
