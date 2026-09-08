#![no_main]

use libfuzzer_sys::fuzz_target;
use serde::de::DeserializeOwned;
use serde_saphyr::{DuplicateKeyPolicy, Error, Options};
use std::collections::BTreeMap;
use std::fmt::{Debug, Write};

const POLICIES: [DuplicateKeyPolicy; 3] = [
    DuplicateKeyPolicy::Error,
    DuplicateKeyPolicy::FirstWins,
    DuplicateKeyPolicy::LastWins,
];

// Keep arbitrary text inside one scalar, including YAML-specific line breaks,
// control characters and non-BMP Unicode. Rust's Debug escaping is not YAML.
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
    let options: Options = serde_saphyr::options! { duplicate_keys: policy };
    if reader {
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options)
    } else {
        serde_saphyr::from_str_with_options(yaml, options)
    }
}

fn check_duplicate<K: DeserializeOwned + Ord + Debug>(
    yaml: &str,
    first_key: K,
    last_key: K,
    first_value: Vec<String>,
    last_value: Vec<String>,
    reader: bool,
) {
    let error = parse::<BTreeMap<K, Vec<String>>>(yaml, DuplicateKeyPolicy::Error, reader)
        .expect_err("equivalent YAML keys must be rejected by the Error policy");
    assert!(
        matches!(error.without_snippet(), Error::DuplicateMappingKey { .. }),
        "expected duplicate-key error for {yaml:?}: {error:?}"
    );

    for (policy, key, value) in [
        (DuplicateKeyPolicy::FirstWins, first_key, first_value),
        (DuplicateKeyPolicy::LastWins, last_key, last_value),
    ] {
        let actual: BTreeMap<K, Vec<String>> = parse(yaml, policy, reader)
            .unwrap_or_else(|error| panic!("valid duplicate mapping under {policy:?}: {error:?}"));
        assert_eq!(
            actual,
            BTreeMap::from([(key, value)]),
            "{policy:?}: {yaml:?}"
        );
    }
}

fn check_distinct<K: DeserializeOwned + Ord + Debug>(
    yaml: &str,
    expected: BTreeMap<K, Vec<String>>,
    reader: bool,
) {
    for policy in POLICIES {
        let actual: BTreeMap<K, Vec<String>> = parse(yaml, policy, reader)
            .unwrap_or_else(|error| panic!("distinct null and string keys: {error:?}"));
        assert_eq!(actual, expected, "{policy:?}: {yaml:?}");
    }
}

// The low four bits choose a scenario; bit 7 selects the reader entry point.
// Remaining bytes provide both arbitrary text and a little-endian u128. Every
// input produces bounded, valid YAML and checks a known semantic result.
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
    let number = u128::from_le_bytes(std::array::from_fn(|i| {
        payload.get(i).copied().unwrap_or_default()
    }));
    let first = format!("0x{number:X}");
    let last = number.to_string();

    match (control & 0x0f) % 6 {
        0 => {
            let key = format!("key:{text}");
            let key_yaml = quoted(&key);
            let yaml = if control & 0x10 == 0 {
                format!("? {key_yaml}\n: {first_yaml}\n? {key_yaml}\n: {last_yaml}\n")
            } else {
                format!("{{? {key_yaml}: {first_yaml}, ? {key_yaml}: {last_yaml}}}\n")
            };
            check_duplicate(&yaml, key.clone(), key, first_value, last_value, reader);
        }
        1 => {
            let yaml = format!("{first}: {first_yaml}\n{last}: {last_yaml}\n");
            check_duplicate(&yaml, first, last, first_value, last_value, reader);
        }
        2 => {
            let yaml =
                format!("? [{first}, {q}]\n: {first_yaml}\n? [{last}, {q}]\n: {last_yaml}\n");
            check_duplicate(
                &yaml,
                vec![first, text.to_string()],
                vec![last, text.to_string()],
                first_value,
                last_value,
                reader,
            );
        }
        3 => {
            // Integer identity also applies to values nested inside map keys.
            let yaml = format!(
                "? {{ids: [{first}, {q}]}}\n: {first_yaml}\n? {{ids: [{last}, {q}]}}\n: {last_yaml}\n"
            );
            check_duplicate(
                &yaml,
                BTreeMap::from([("ids".to_owned(), vec![first, text.to_string()])]),
                BTreeMap::from([("ids".to_owned(), vec![last, text.to_string()])]),
                first_value,
                last_value,
                reader,
            );
        }
        4 => {
            let spellings = [
                ("null", "!!str null", "null"),
                ("~", "'~'", "~"),
                ("NULL", "'NULL'", "NULL"),
                ("!!null null", "'null'", "null"),
                ("!!null 'null'", "! null", "null"),
                ("", "''", ""),
            ];
            let (null, string, value) = spellings
                [usize::from(payload.first().copied().unwrap_or_default()) % spellings.len()];
            // Replay through a merge exercises the same key identity after buffering.
            let prefix = if control & 0x20 == 0 { "" } else { "<<:\n" };
            let yaml = format!(
                "{prefix}  ? {{{null}: {q}}}\n  : {first_yaml}\n  ? {{{string}: {q}}}\n  : {last_yaml}\n"
            );
            check_distinct(
                &yaml,
                BTreeMap::from([
                    (BTreeMap::from([(None, text.to_string())]), first_value),
                    (
                        BTreeMap::from([(Some(value.to_owned()), text.to_string())]),
                        last_value,
                    ),
                ]),
                reader,
            );
        }
        _ => {
            let yaml =
                format!("? [null, {q}]\n: {first_yaml}\n? [!!null 'null', {q}]\n: {last_yaml}\n");
            let key = vec![None, Some(text.to_string())];
            check_duplicate(&yaml, key.clone(), key, first_value, last_value, reader);
        }
    }
});
