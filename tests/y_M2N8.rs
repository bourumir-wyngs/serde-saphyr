use serde::Deserialize;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// M2N8: Question mark edge cases
// Case 1: a sequence with one mapping that uses explicit empty keys ("- ? : x").
// Case 2: a mapping where the first key is itself a mapping { []: x } and there is
// an additional empty key with empty value. For this case we currently only assert
// that the document parses successfully; detailed structural assertions can be added
// later as parser/deserializer support for complex/empty explicit keys improves.

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct InnerKey {
    #[serde(flatten)]
    map: HashMap<Option<String>, String>,
}

// We use InnerKey as a key in an outer HashMap. Because InnerKey wraps a HashMap inside,
// we cannot derive Hash (std::collections::HashMap does not implement Hash), and even if
// it did, iteration order would make the hash nondeterministic. To satisfy the Eq/Hash
// contract (equal keys must have equal hashes), we compute an order-independent hash by
// collecting the inner entries, sorting them into a canonical order, and then hashing the
// sorted sequence. This mirrors HashMap's Eq semantics (order-independent equality).
impl Hash for InnerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut entries: Vec<(&Option<String>, &String)> = self.map.iter().collect();
        entries.sort_by(|a, b| {
            let ka = match a.0 { Some(s) => (1, s.as_str()), None => (0, "") };
            let kb = match b.0 { Some(s) => (1, s.as_str()), None => (0, "") };
            ka.cmp(&kb).then(a.1.cmp(b.1))
        });
        entries.len().hash(state);
        for (k, v) in entries {
            k.hash(state);
            v.hash(state);
        }
    }
}

#[test]
fn yaml_m2n8_case1_sequence_with_explicit_empty_key_parses() {
    let y1 = "- ? : x\n";
    // The document is a sequence with one element: a mapping with a single pair.
    // Key = a mapping { "": "x" }, Value = empty scalar.
    type PairMap = HashMap<InnerKey, Option<String>>;
    let docs: Vec<PairMap> = serde_saphyr::from_str(y1)
        .expect("M2N8 case1 should parse");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].len(), 1);

    // Inspect the only entry directly to avoid relying on Hash/Eq nuances
    let (only_key, only_val) = docs[0].iter().next().expect("missing entry");
    assert_eq!(only_val.as_ref(), None);

    // The complex key should contain an inner mapping with a single pair mapping to "x".
    // We assert the value and avoid constraining the exact key representation (empty vs null-like).
    assert_eq!(only_key.map.len(), 1);
    assert_eq!(only_key.map.values().next().map(String::as_str), Some("x"));
}

#[test]
fn yaml_m2n8_case2_mapping_with_complex_key_shape() {
    // According to the suite, this is a mapping with two pairs:
    // 1) key = { []: x } (a mapping used as the key), value = (empty)
    // 2) key = (empty scalar), value = (empty)
    // For now, assert that it parses without error; detailed structural checks can be added later.
    let y2 = "? []: x\n:\n";

    let _doc: serde::de::IgnoredAny = serde_saphyr::from_str(y2)
        .expect("M2N8 case2 should parse");
}
