use serde::Deserialize;

/// Minimal value tree preserving nulls and collections used as mapping keys.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Canon {
    Null,
    String(String),
    Seq(Vec<Canon>),
    Map(Vec<(Canon, Canon)>),
}

impl<'de> Deserialize<'de> for Canon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Canon;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any YAML value")
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Canon::Null)
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Canon::Null)
            }
            fn visit_some<D2>(self, d: D2) -> Result<Self::Value, D2::Error>
            where
                D2: serde::Deserializer<'de>,
            {
                Canon::deserialize(d)
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Canon::String(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(Canon::String(v))
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(elem) = seq.next_element::<Canon>()? {
                    out.push(elem);
                }
                Ok(Canon::Seq(out))
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = Vec::new();
                while let Some((k, v)) = map.next_entry::<Canon, Canon>()? {
                    entries.push((k, v));
                }
                Ok(Canon::Map(entries))
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[test]
fn yaml_m2n8_case1_sequence_preserves_mapping_key_with_null_inner_key() {
    // M2N8:00 is [{ {null: x}: null }]. The explicit key is the whole inner map.
    let yaml = "- ? : x\n";
    let doc: Canon = serde_saphyr::from_str(yaml).expect("M2N8:00 should parse");

    assert_eq!(
        doc,
        Canon::Seq(vec![Canon::Map(vec![(
            Canon::Map(vec![(Canon::Null, Canon::String("x".to_owned()))]),
            Canon::Null,
        )])]),
    );
}

#[test]
fn yaml_m2n8_case2_preserves_mapping_key_with_empty_sequence_inner_key() {
    // M2N8:01 is { {[]: x}: null }, with exactly one outer mapping entry.
    let yaml = "? []: x\n";
    let doc: Canon = serde_saphyr::from_str(yaml).expect("M2N8:01 should parse");

    assert_eq!(
        doc,
        Canon::Map(vec![(
            Canon::Map(vec![(Canon::Seq(vec![]), Canon::String("x".to_owned()))]),
            Canon::Null,
        )]),
    );
}
