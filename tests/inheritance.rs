use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Debug, Ord)]
pub struct KeyString(String);

#[derive(Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Debug, Ord)]
pub struct KeyUsized(usize);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TwoMaps {
    // The value in the map doesn't matter here
    pub my_hash_map: HashMap<KeyString, u16>,
    pub my_btree_map: BTreeMap<KeyString, u16>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TwoMapsUsized {
    // The value in the map doesn't matter here
    pub my_hash_map: HashMap<KeyUsized, u16>,
    pub my_btree_map: BTreeMap<KeyUsized, u16>,
}

#[test]
fn test_map() -> anyhow::Result<()> {
    let yaml = r#"my_hash_map:
  my_key_1: 1
my_btree_map:
  my_key_3: 3
  my_key_4: 4
"#;

    let my_struct = serde_saphyr::from_str::<TwoMaps>(yaml).unwrap();

    let mut buffer = String::new();
    serde_saphyr::to_fmt_writer(&mut buffer, &my_struct).unwrap();

    println!("Deserialized:\n{buffer}");

    // Newtype keys that wrap scalars should serialize as scalar keys, not as YAML complex keys.
    assert!(!buffer.contains("\n? "));

    // With a single HashMap entry the output is deterministic, so assert the full YAML.
    assert_eq!(buffer, yaml);

    // Round-trip: YAML -> struct -> YAML -> struct
    let my_struct_rt = serde_saphyr::from_str::<TwoMaps>(&buffer)?;
    assert_eq!(my_struct, my_struct_rt);
    Ok(())
}

#[test]
fn test_map_usized() -> anyhow::Result<()> {
    let yaml = r#"my_hash_map:
  1000: 1
my_btree_map:
  3000: 3
  4000: 4
"#;

    let my_struct = serde_saphyr::from_str::<TwoMapsUsized>(yaml).unwrap();

    let mut buffer = String::new();
    serde_saphyr::to_fmt_writer(&mut buffer, &my_struct).unwrap();
    assert!(!buffer.contains("\n? "));

    // With a single HashMap entry the output is deterministic, so assert the full YAML.
    assert_eq!(buffer, yaml);

    let my_struct_rt = serde_saphyr::from_str::<TwoMapsUsized>(&buffer)?;
    assert_eq!(my_struct, my_struct_rt);
    Ok(())
}

#[test]
fn test_string() -> anyhow::Result<()> {
    let key = KeyString("the_key".to_string());
    let mut buffer = String::new();
    serde_saphyr::to_fmt_writer(&mut buffer, &key).unwrap();
    let deserialized: KeyString = serde_saphyr::from_str(&buffer[..])?;
    assert_eq!(key, deserialized);
    assert_eq!(
        buffer,
        r#"the_key
"#
    );
    Ok(())
}
