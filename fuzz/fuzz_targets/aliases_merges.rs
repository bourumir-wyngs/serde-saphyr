#![no_main]

use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, de::DeserializeOwned};
use serde_saphyr::{DuplicateKeyPolicy, Error, Options};
use std::collections::BTreeMap;
use std::fmt::Write;

type Mapping = BTreeMap<String, Vec<String>>;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct AliasDoc {
    scalar: String,
    scalar_alias: String,
    sequence: Vec<String>,
    sequence_alias: Vec<String>,
    mapping: Mapping,
    mapping_alias: Mapping,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct MergeDoc {
    base1: Mapping,
    base2: Mapping,
    merged: Mapping,
}

const POLICIES: [DuplicateKeyPolicy; 3] = [
    DuplicateKeyPolicy::Error,
    DuplicateKeyPolicy::FirstWins,
    DuplicateKeyPolicy::LastWins,
];

// Escaping non-ASCII characters also preserves YAML's special line breaks.
fn quoted(text: &str) -> String {
    let mut result = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            ' '..='~' => result.push(ch),
            _ => write!(result, "\\U{:08X}", u32::from(ch)).unwrap(),
        }
    }
    result.push('"');
    result
}

fn parse<T: DeserializeOwned>(
    yaml: &str,
    policy: DuplicateKeyPolicy,
    reader: bool,
) -> Result<T, Error> {
    let options: Options = serde_saphyr::options! {
        duplicate_keys: policy,
        budget: serde_saphyr::budget! {
            max_depth: 32,
            max_events: 4096,
            max_total_scalar_bytes: 1024 * 1024,
        },
        alias_limits: serde_saphyr::alias_limits! {
            max_total_replayed_events: 4096,
            max_replay_stack_depth: 16,
            max_alias_expansions_per_anchor: 512,
        },
    };
    if reader {
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options)
    } else {
        serde_saphyr::from_str_with_options(yaml, options)
    }
}

// Low four bits select aliases, merges, duplicate anchors, or raw syntax. Bit 7
// selects reader input; merge flags select explicit-key position (4), reversed
// source order (5), and an explicit override (6). All other bytes are payload.
fuzz_target!(|data: &[u8]| {
    if data.len() > 4096 {
        return;
    }
    let control = data.first().copied().unwrap_or_default();
    let reader = control & 0x80 != 0;
    let payload = data.get(1..).unwrap_or_default();
    let text = String::from_utf8_lossy(payload);
    let q = quoted(&text);
    let first_value = vec![text.to_string(), "first".to_owned()];
    let last_value = vec!["last".to_owned(), text.to_string(), "tail".to_owned()];
    let first_yaml = format!("[{q}, first]");
    let last_yaml = format!("[last, {q}, tail]");

    match (control & 0x0f) % 4 {
        0 => {
            let yaml = format!(
                "scalar: &S {q}\nscalar_alias: *S\nsequence: &Q [*S, second, *S]\nsequence_alias: *Q\nmapping: &M {{key: *Q, other: [tail]}}\nmapping_alias: *M\n"
            );
            let sequence = vec![text.to_string(), "second".to_owned(), text.to_string()];
            let mapping = BTreeMap::from([
                ("key".to_owned(), sequence.clone()),
                ("other".to_owned(), vec!["tail".to_owned()]),
            ]);
            let expected = AliasDoc {
                scalar: text.to_string(),
                scalar_alias: text.to_string(),
                sequence: sequence.clone(),
                sequence_alias: sequence,
                mapping: mapping.clone(),
                mapping_alias: mapping,
            };
            for policy in POLICIES {
                let actual: AliasDoc =
                    parse(&yaml, policy, reader).expect("valid anchored document");
                assert_eq!(actual, expected, "{policy:?}: {yaml:?}");
            }
        }
        1 => {
            let number = u128::from_le_bytes(std::array::from_fn(|i| {
                payload.get(i).copied().unwrap_or_default()
            }));
            let first_key = format!("0x{number:X}");
            let last_key = number.to_string();
            let explicit_key = format!("0o{number:o}");
            let reversed = control & 0x20 != 0;
            let explicit = control & 0x40 != 0;
            let sources = if reversed { "*B2, *B1" } else { "*B1, *B2" };
            let mut entries = vec![format!("<<: [{sources}]"), format!("extra: [{q}]")];
            if explicit {
                let override_entry = format!("{explicit_key}: [explicit, {q}]");
                if control & 0x10 == 0 {
                    entries.insert(0, override_entry);
                } else {
                    entries.push(override_entry);
                }
            }
            let yaml = format!(
                "base1: &B1 {{{first_key}: {first_yaml}, left: [{q}]}}\nbase2: &B2 {{{last_key}: {last_yaml}, right: [{q}]}}\nmerged: {{{}}}\n",
                entries.join(", ")
            );
            let base1 = BTreeMap::from([
                (first_key.clone(), first_value.clone()),
                ("left".to_owned(), vec![text.to_string()]),
            ]);
            let base2 = BTreeMap::from([
                (last_key.clone(), last_value.clone()),
                ("right".to_owned(), vec![text.to_string()]),
            ]);
            let (winning_key, winning_value) = if explicit {
                (explicit_key, vec!["explicit".to_owned(), text.to_string()])
            } else if reversed {
                (last_key, last_value)
            } else {
                (first_key, first_value)
            };
            let expected = MergeDoc {
                base1,
                base2,
                merged: BTreeMap::from([
                    (winning_key, winning_value),
                    ("left".to_owned(), vec![text.to_string()]),
                    ("right".to_owned(), vec![text.to_string()]),
                    ("extra".to_owned(), vec![text.to_string()]),
                ]),
            };
            for policy in POLICIES {
                let actual: MergeDoc = parse(&yaml, policy, reader).expect("valid merged document");
                assert_eq!(actual, expected, "{policy:?}: {yaml:?}");
            }
        }
        2 => {
            // Skipping or replacing a duplicate must still register its anchor.
            let yaml = format!(
                "selected: &first {first_yaml}\nselected: &last {last_yaml}\nfirst_copy: *first\nlast_copy: *last\n"
            );
            let error = parse::<Mapping>(&yaml, DuplicateKeyPolicy::Error, reader)
                .expect_err("duplicate selected key");
            assert!(matches!(
                error.without_snippet(),
                Error::DuplicateMappingKey { .. }
            ));
            for (policy, value) in [
                (DuplicateKeyPolicy::FirstWins, first_value.clone()),
                (DuplicateKeyPolicy::LastWins, last_value.clone()),
            ] {
                let actual: Mapping =
                    parse(&yaml, policy, reader).expect("valid duplicate anchors");
                let expected = BTreeMap::from([
                    ("selected".to_owned(), value),
                    ("first_copy".to_owned(), first_value.clone()),
                    ("last_copy".to_owned(), last_value.clone()),
                ]);
                assert_eq!(actual, expected, "{policy:?}: {yaml:?}");
            }
        }
        _ => {
            // Retain malformed-input panic coverage alongside the semantic cases.
            // Raw interpolation is deliberate only in this branch.
            let policy = POLICIES[usize::from(control >> 4) % POLICIES.len()];
            let yaml = format!(
                "scalar: &S {text}\nscalar_alias: *S\nsequence: &Q [{text}]\nsequence_alias: *Q\nmapping: &M {{{text}}}\nmapping_alias: *M\n"
            );
            let _ = parse::<AliasDoc>(&yaml, policy, reader);
            let yaml = format!(
                "base1: &B1 {{{text}}}\nbase2: &B2 {{{text}}}\nmerged: {{<<: [*B1, *B2]}}\n"
            );
            let _ = parse::<MergeDoc>(&yaml, policy, reader);
        }
    }
});
