#![cfg(feature = "deserialize")]

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;

use rstest::rstest;
use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, Visitor};
use serde_saphyr::{DuplicateKeyPolicy, Error, from_str_with_options, options};

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    #[serde(rename = "str")]
    Str(String),
    #[serde(rename = "int")]
    Int(u8),
    #[serde(rename = "map")]
    Map(BTreeMap<String, String>),
    #[serde(rename = "seq")]
    Seq(Vec<Key>),
    #[serde(rename = "merge")]
    Merge(String),
    #[serde(rename = "foo")]
    Foo,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "<<")]
    MergeText,
}

/// `local` and `ordinary` are YAML spellings with expected `local_key` and
/// `ordinary_key` values. `duplicate_keys` selects the policy and `merged` selects
/// merge expansion. Returns normally when both distinct entries are retained.
#[rstest]
#[case::string("!str foo", "!!str foo", Key::Str("foo".into()), Key::Foo)]
#[case::integer("!int 1", "1", Key::Int(1), Key::One)]
#[case::mapping("!map {str: foo}", "!!map {str: foo}", Key::Map(BTreeMap::from([("str".into(), "foo".into())])), Key::Str("foo".into()))]
#[case::merge("!merge <<", "'<<'", Key::Merge("<<".into()), Key::MergeText)]
fn local_enum_selectors_keep_distinct_key_identities(
    #[case] local: &str,
    #[case] ordinary: &str,
    #[case] local_key: Key,
    #[case] ordinary_key: Key,
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    duplicate_keys: DuplicateKeyPolicy,
    #[values(false, true)] merged: bool,
) {
    let entries = format!("? {local}\n: 1\n? {ordinary}\n: 2\n");
    let yaml = if merged {
        format!(
            "<<:\n{}",
            entries
                .lines()
                .map(|line| format!("  {line}\n"))
                .collect::<String>()
        )
    } else {
        entries
    };
    let actual: BTreeMap<Key, u8> =
        from_str_with_options(&yaml, options! { duplicate_keys: duplicate_keys }).unwrap();
    assert_eq!(actual, BTreeMap::from([(local_key, 1), (ordinary_key, 2)]));
}

/// `duplicate_keys` chooses the winner, `expected` is its value, and `merged`
/// enables merge expansion. Returns normally when repeated keys follow the policy.
#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, 1)]
#[case(DuplicateKeyPolicy::LastWins, 2)]
fn repeated_enum_keys_follow_duplicate_policy(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: u8,
    #[values(false, true)] merged: bool,
) {
    let entries = "? !seq [!int 1]\n: 1\n? !seq [!int 1]\n: 2\n";
    let yaml = if merged {
        format!(
            "<<:\n{}",
            entries
                .lines()
                .map(|line| format!("  {line}\n"))
                .collect::<String>()
        )
    } else {
        entries.to_owned()
    };
    let actual: BTreeMap<Key, u8> =
        from_str_with_options(&yaml, options! { duplicate_keys: duplicate_keys }).unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([(Key::Seq(vec![Key::Int(1)]), expected)])
    );
    let error = from_str_with_options::<BTreeMap<Key, u8>>(
        &yaml,
        options! { duplicate_keys: DuplicateKeyPolicy::Error },
    )
    .unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
}

#[test]
fn ordinary_newtype_keys_retain_enum_context() {
    #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Wrapped(Key);

    let actual: BTreeMap<Wrapped, u8> =
        serde_saphyr::from_str("!str foo: 1\n!!str foo: 2\n").unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([(Wrapped(Key::Str("foo".into())), 1), (Wrapped(Key::Foo), 2)])
    );
}

#[rstest]
#[case("<<: !int {x: 1}")]
#[case("<<: !str [{x: 1}]")]
#[case("<<: [!int {x: 1}]")]
#[case("<<: {<<: !int {x: 1}}")]
fn merge_containers_still_validate_tag_shapes(#[case] yaml: &str) {
    let error = serde_saphyr::from_str::<BTreeMap<String, u8>>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[derive(Debug, PartialEq)]
struct CountedKeys {
    calls: usize,
    keys: Vec<Key>,
}

impl<'de> Deserialize<'de> for CountedKeys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeySeed<'a>(&'a Cell<usize>);
        impl<'de> DeserializeSeed<'de> for KeySeed<'_> {
            type Value = Key;
            fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Key, D::Error> {
                self.0.set(self.0.get() + 1);
                Key::deserialize(deserializer)
            }
        }
        struct CountVisitor;
        impl<'de> Visitor<'de> for CountVisitor {
            type Value = CountedKeys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<CountedKeys, M::Error> {
                let calls = Cell::new(0);
                let mut keys = Vec::new();
                while let Some(key) = map.next_key_seed(KeySeed(&calls))? {
                    keys.push(key);
                    let _: u8 = map.next_value()?;
                }
                Ok(CountedKeys {
                    calls: calls.get(),
                    keys,
                })
            }
        }
        deserializer.deserialize_map(CountVisitor)
    }
}

/// `yaml` is the mapping, `calls` is the expected seed invocation count, and
/// `keys` is the expected output. Returns normally when seeds are consumed once.
#[rstest]
#[case::empty("{}", 0, vec![])]
#[case::two_keys("!str foo: 1\n!!str foo: 2\n", 2, vec![Key::Str("foo".into()), Key::Foo])]
#[case::trailing_duplicates("!str foo: 1\n!str foo: 2\n", 1, vec![Key::Str("foo".into())])]
fn key_seeds_are_never_probed_or_reused(
    #[case] yaml: &str,
    #[case] calls: usize,
    #[case] keys: Vec<Key>,
) {
    let actual: CountedKeys = from_str_with_options(
        yaml,
        options! { duplicate_keys: DuplicateKeyPolicy::FirstWins },
    )
    .unwrap();
    assert_eq!(actual, CountedKeys { calls, keys });
}

#[rstest]
fn nested_local_enum_selectors_keep_distinct_identities(
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    duplicate_keys: DuplicateKeyPolicy,
) {
    let opts = options! { duplicate_keys: duplicate_keys };
    let optional: BTreeMap<Option<Key>, u8> =
        from_str_with_options("!str foo: 1\n!!str foo: 2\n", opts.clone()).unwrap();
    assert_eq!(
        optional,
        BTreeMap::from([(Some(Key::Str("foo".into())), 1), (Some(Key::Foo), 2)])
    );

    let tuples: BTreeMap<(Key,), u8> =
        from_str_with_options("? [!str foo]\n: 1\n? [!!str foo]\n: 2\n", opts.clone()).unwrap();
    assert_eq!(
        tuples,
        BTreeMap::from([((Key::Str("foo".into()),), 1), ((Key::Foo,), 2)])
    );

    let nested: BTreeMap<Key, u8> =
        from_str_with_options("? !seq [!str foo]\n: 1\n? !seq [!!str foo]\n: 2\n", opts).unwrap();
    assert_eq!(
        nested,
        BTreeMap::from([
            (Key::Seq(vec![Key::Str("foo".into())]), 1),
            (Key::Seq(vec![Key::Foo]), 2)
        ])
    );
}

#[rstest]
fn local_integer_named_variants_preserve_string_payloads(
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    duplicate_keys: DuplicateKeyPolicy,
) {
    #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    enum TextKey {
        #[serde(rename = "int")]
        Int(String),
    }
    let actual: BTreeMap<TextKey, u8> = from_str_with_options(
        "!int 0xB: 1\n!int 11: 2\n",
        options! { duplicate_keys: duplicate_keys },
    )
    .unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            (TextKey::Int("0xB".into()), 1),
            (TextKey::Int("11".into()), 2)
        ])
    );
}

#[rstest]
#[case("!str foo: 1\n!<!str> foo: 2\n")]
#[case("%TAG !local! !\n---\n!str foo: 1\n!local!str foo: 2\n")]
fn equivalent_resolved_local_tags_are_duplicate_keys(#[case] yaml: &str) {
    let error = serde_saphyr::from_str::<BTreeMap<Key, u8>>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
}

#[rstest]
fn local_and_global_tags_keep_distinct_yaml_identity_for_ordinary_keys(
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    duplicate_keys: DuplicateKeyPolicy,
) {
    // Duplicate policies compare YAML key identities before conversion.
    // !str foo and !!str foo are distinct YAML keys, so neither filters the
    // other. Both become the Rust String "foo"; BTreeMap's later insertion
    // replaces the earlier value even under FirstWins.
    let actual: BTreeMap<String, u8> = from_str_with_options(
        "!str foo: 1\n!!str foo: 2\n",
        options! { duplicate_keys: duplicate_keys },
    )
    .unwrap();
    assert_eq!(actual, BTreeMap::from([("foo".into(), 2)]));
}

#[rstest]
#[case::error(DuplicateKeyPolicy::Error, None)]
#[case::first(DuplicateKeyPolicy::FirstWins, Some(2))]
#[case::last(DuplicateKeyPolicy::LastWins, Some(3))]
fn duplicate_policy_precedes_rust_key_conversion(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: Option<u8>,
) {
    // !str a and a are distinct YAML keys but become the same Rust String.
    // Only the second and third entries are YAML duplicates. FirstWins drops
    // a: 3, then BTreeMap's insertion of a: 2 replaces the value from !str a: 1.
    let actual = from_str_with_options::<BTreeMap<String, u8>>(
        "{!str a: 1, a: 2, a: 3}",
        options! { duplicate_keys: duplicate_keys },
    );
    match expected {
        Some(value) => assert_eq!(actual.unwrap(), BTreeMap::from([("a".into(), value)])),
        None => assert!(matches!(
            actual.unwrap_err().without_snippet(),
            Error::DuplicateMappingKey { .. }
        )),
    }
}
