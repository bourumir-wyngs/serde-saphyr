use serde_json::Value;
use serde_saphyr::{DuplicateKeyPolicy, Options};
use std::collections::BTreeMap;

#[test]
fn yaml_x38w_aliases_in_flow_objects_with_complex_keys() -> anyhow::Result<()> {
    let y = r#"{ &a [a, &b b]: *b, *a : [c, *b, d]}"#;

    // Use BTreeMap<Vec<String>, serde_json::Value> to allow complex (sequence) keys
    // and heterogeneous values (scalar first, then sequence overriding it).
    let map: BTreeMap<Vec<String>, Value> = serde_saphyr::from_str_with_options(
        y,
        Options {
            // With serde-saphyr, we need to configure correct duplicate key policy for this to work
            // Default policy would be rejecting duplicate keys.
            duplicate_keys: DuplicateKeyPolicy::LastWins,
            ..Default::default()
        },
    )?;

    assert_eq!(
        map.len(),
        1,
        "duplicate complex keys should collapse to one entry (last wins)"
    );

    let key = vec!["a".to_string(), "b".to_string()];
    let val = map.get(&key).expect("expected key [a, b]");
    let arr = val
        .as_array()
        .expect("final value should be a sequence after override");
    let got: Vec<String> = arr
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, vec!["c", "b", "d"]);
    Ok(())
}
