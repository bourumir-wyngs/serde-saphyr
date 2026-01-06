use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum WithVector {
    Retrieve {
        vector: Vec<String>,
        map: BTreeMap<String, String>,
    },
}

#[test]
fn block_sequence_value_must_not_make_next_map_inline() -> anyhow::Result<()> {
    let mut map = BTreeMap::new();
    map.insert("h_key".to_string(), "h_value".to_string());

    let with_vector = WithVector::Retrieve {
        vector: vec!["element1".to_string(), "element2".to_string()],
        map,
    };

    let serialized = serde_saphyr::to_string(&with_vector)?;

    // Regression: a block sequence value must not cause the following mapping value
    // to be emitted inline as `map: h_key: h_value`.
    assert!(
        serialized.contains("map:\n    h_key: h_value\n"),
        "Unexpected YAML:\n{serialized}"
    );
    assert!(
        !serialized.contains("map: h_key: h_value"),
        "Unexpected YAML:\n{serialized}"
    );

    // And ensure it round-trips.
    let decoded: WithVector = serde_saphyr::from_str(&serialized)?;
    assert_eq!(decoded, with_vector);

    Ok(())
}
