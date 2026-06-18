#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;
use std::rc::Rc;

use serde::Serialize;
use serde_saphyr::{FlowMap, FlowSeq, NullPolicy, RcAnchor, ser_options, to_string_with_options};

#[derive(Serialize)]
struct BlockNulls {
    none: Option<u8>,
    unit: (),
}

#[test]
fn null_policy_controls_block_null_spelling() -> anyhow::Result<()> {
    let value = BlockNulls {
        none: None,
        unit: (),
    };

    let nulls = to_string_with_options(&value, ser_options! { null_policy: NullPolicy::NullNull })?;
    assert_eq!(nulls, "none: null\nunit: null\n");

    let tildes =
        to_string_with_options(&value, ser_options! { null_policy: NullPolicy::NullTilde })?;
    assert_eq!(tildes, "none: ~\nunit: ~\n");

    let empty =
        to_string_with_options(&value, ser_options! { null_policy: NullPolicy::NullEmpty })?;
    assert_eq!(empty, "none:\nunit:\n");

    Ok(())
}

#[test]
fn null_empty_falls_back_to_null_in_flow_collections() -> anyhow::Result<()> {
    let options = ser_options! { null_policy: NullPolicy::NullEmpty };

    let seq = to_string_with_options(&FlowSeq(vec![None::<u8>, Some(1)]), options)?;
    assert_eq!(seq, "[null, 1]\n");
    let seq_back: Vec<Option<u8>> = serde_saphyr::from_str(&seq)?;
    assert_eq!(seq_back, vec![None, Some(1)]);

    let mut map = BTreeMap::new();
    map.insert("a", None::<u8>);
    map.insert("b", Some(2));

    let flow_map = to_string_with_options(&FlowMap(map), options)?;
    assert_eq!(flow_map, "{a: null, b: 2}\n");
    let map_back: BTreeMap<String, Option<u8>> = serde_saphyr::from_str(&flow_map)?;
    assert_eq!(map_back.get("a"), Some(&None));
    assert_eq!(map_back.get("b"), Some(&Some(2)));

    Ok(())
}

#[test]
fn null_empty_serializes_top_level_null_as_empty_scalar() -> anyhow::Result<()> {
    let yaml = to_string_with_options(
        &None::<u8>,
        ser_options! { null_policy: NullPolicy::NullEmpty },
    )?;
    assert_eq!(yaml, "\n");

    let parsed: Option<u8> = serde_saphyr::from_str(&yaml)?;
    assert_eq!(parsed, None);

    Ok(())
}

#[derive(Serialize)]
struct AnchoredNulls {
    value: RcAnchor<Option<u8>>,
    alias: RcAnchor<Option<u8>>,
}

#[test]
fn null_empty_keeps_anchor_prefix_on_empty_map_value() -> anyhow::Result<()> {
    let shared = Rc::new(None::<u8>);
    let value = AnchoredNulls {
        value: RcAnchor(shared.clone()),
        alias: RcAnchor(shared),
    };

    let yaml = to_string_with_options(&value, ser_options! { null_policy: NullPolicy::NullEmpty })?;

    assert_eq!(yaml, concat!("value: &a1 ", "\nalias: *a1\n"));

    Ok(())
}
