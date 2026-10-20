#![cfg(feature = "deserialize")]

use std::{collections::BTreeMap, fmt, marker::PhantomData};

use rstest::rstest;
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, MapAccess, Visitor},
};
use serde_saphyr::{DuplicateKeyPolicy, Error, MergeKeyPolicy};

#[derive(Debug, PartialEq)]
struct Pairs<K, V>(Vec<(K, V)>);

impl<'de, K: Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de> for Pairs<K, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PairsVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K: Deserialize<'de>, V: Deserialize<'de>> Visitor<'de> for PairsVisitor<K, V> {
            type Value = Pairs<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a mapping whose entries are preserved")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut pairs = Vec::new();
                while let Some(pair) = map.next_entry()? {
                    pairs.push(pair);
                }
                Ok(Pairs(pairs))
            }
        }

        deserializer.deserialize_map(PairsVisitor(PhantomData))
    }
}

fn parse<T: DeserializeOwned>(yaml: &str, reader: bool) -> Result<T, Error> {
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::PassThrough };
    if reader {
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options)
    } else {
        serde_saphyr::from_str_with_options(yaml, options)
    }
}

fn owned_pairs(pairs: &[(&str, &str)]) -> Pairs<String, String> {
    Pairs(
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    )
}

#[rstest]
#[case::block(("hi: first\nhi: second\n", ["hi", "hi"]))]
#[case::flow(("{hi: first, hi: second}", ["hi", "hi"]))]
#[case::numeric(("0xB: first\n11: second\n", ["0xB", "11"]))]
fn repeated_entries_reach_custom_visitor(
    #[case] input: (&str, [&str; 2]),
    #[values(false, true)] reader: bool,
) {
    let (yaml, keys) = input;
    let actual: Pairs<String, String> = parse(yaml, reader).unwrap();
    assert_eq!(
        actual,
        owned_pairs(&[(keys[0], "first"), (keys[1], "second")])
    );
}

#[rstest]
fn derived_struct_rejects_duplicate_fields(#[values(false, true)] reader: bool) {
    #[derive(Debug, Deserialize)]
    struct Record {
        hi: String,
    }

    assert_eq!(parse::<Record>("hi: first", reader).unwrap().hi, "first");
    let error = parse::<Record>("hi: first\nhi: second\n", reader).unwrap_err();
    assert!(!matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
    assert!(
        error.to_string().contains("duplicate field `hi`"),
        "{error}"
    );
}

#[rstest]
fn standard_map_controls_duplicate_handling(#[values(false, true)] reader: bool) {
    let repeated: BTreeMap<String, String> = parse("hi: first\nhi: second\n", reader).unwrap();
    assert_eq!(repeated, BTreeMap::from([("hi".into(), "second".into())]));

    let spellings: BTreeMap<String, String> = parse("0xB: first\n11: second\n", reader).unwrap();
    assert_eq!(
        spellings,
        BTreeMap::from([
            ("0xB".into(), "first".into()),
            ("11".into(), "second".into()),
        ])
    );
    let numbers: BTreeMap<i32, String> = parse("0xB: first\n11: second\n", reader).unwrap();
    assert_eq!(numbers, BTreeMap::from([(11, "second".into())]));
}

#[rstest]
fn composite_keys_are_forwarded(#[values(false, true)] reader: bool) {
    let actual: Pairs<Vec<String>, String> =
        parse("? [0xB, hi]\n: first\n? [11, hi]\n: second\n", reader).unwrap();
    assert_eq!(
        actual,
        Pairs(vec![
            (vec!["0xB".into(), "hi".into()], "first".into()),
            (vec!["11".into(), "hi".into()], "second".into()),
        ])
    );
}

#[rstest]
fn nested_mappings_and_aliases_preserve_entries(#[values(false, true)] reader: bool) {
    #[derive(Debug, Deserialize)]
    struct Root {
        original: Pairs<String, String>,
        copy: Pairs<String, String>,
    }

    let actual: Root = parse(
        "original: &pairs {hi: first, hi: second}\ncopy: *pairs\n",
        reader,
    )
    .unwrap();
    let expected = owned_pairs(&[("hi", "first"), ("hi", "second")]);
    assert_eq!(actual.original, expected);
    assert_eq!(actual.copy, expected);
}

#[test]
fn repeated_entries_can_borrow_strings() {
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::PassThrough };
    let actual: Pairs<&str, &str> =
        serde_saphyr::from_str_with_options("hi: first\nhi: second\n", options).unwrap();
    assert_eq!(actual, Pairs(vec![("hi", "first"), ("hi", "second")]));
}

#[rstest]
#[case::explicit_override(("<<: {hi: inherited}\nhi: first\nhi: second\n", vec![("hi", "first"), ("hi", "second")]))]
#[case::source_precedence(("<<: [{hi: first}, {hi: second, other: value}]\n", vec![("hi", "first"), ("other", "value")]))]
#[case::duplicate_source(("<<: {hi: first, hi: second}\n", vec![("hi", "first")]))]
fn merge_expansion_keeps_its_precedence(
    #[case] input: (&str, Vec<(&str, &str)>),
    #[values(false, true)] reader: bool,
) {
    let (yaml, expected) = input;
    let actual: Pairs<String, String> = parse(yaml, reader).unwrap();
    assert_eq!(actual, owned_pairs(&expected));
}

#[test]
fn ordinary_merge_keys_are_forwarded() {
    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::PassThrough,
        merge_keys: MergeKeyPolicy::AsOrdinary,
    };
    let actual: Pairs<String, String> =
        serde_saphyr::from_str_with_options("<<: first\n<<: second\n", options).unwrap();
    assert_eq!(actual, owned_pairs(&[("<<", "first"), ("<<", "second")]));
}

#[test]
fn default_policy_still_rejects_duplicates() {
    let error =
        serde_saphyr::from_str::<Pairs<String, String>>("hi: first\nhi: second\n").unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
}

#[cfg(feature = "serde_derived_types")]
#[test]
fn policy_uses_snake_case_in_configuration() {
    assert_eq!(
        serde_json::to_string(&DuplicateKeyPolicy::PassThrough).unwrap(),
        "\"pass_through\""
    );
    assert!(matches!(
        serde_json::from_str::<DuplicateKeyPolicy>("\"pass_through\"").unwrap(),
        DuplicateKeyPolicy::PassThrough
    ));
}
