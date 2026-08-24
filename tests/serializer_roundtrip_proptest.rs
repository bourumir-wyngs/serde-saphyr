#![cfg(all(feature = "serialize", feature = "deserialize"))]

use proptest::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Value {
    Null,
    Bool(bool),
    Int(i64),
    String(String),
    Seq(Vec<Value>),
    Map(Vec<(String, Value)>),
}

fn arb_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::Int),
        any::<String>().prop_map(Value::String),
    ];

    leaf.prop_recursive(
        8,   // maximum recursion depth
        256, // maximum total generated size
        16,  // maximum items per collection
        |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..16).prop_map(Value::Seq),
                prop::collection::vec((any::<String>(), inner), 0..16).prop_map(Value::Map),
            ]
        },
    )
}

fn roundtrip_config() -> ProptestConfig {
    let mut config = ProptestConfig::default();

    if cfg!(any(miri, target_arch = "wasm32")) {
        config.failure_persistence = None;
    }

    if cfg!(miri) {
        config.cases = 16;
    }

    config
}

proptest! {
    #![proptest_config(roundtrip_config())]

    #[test]
    fn serializer_output_roundtrips(value in arb_value()) {
        let yaml = serde_saphyr::to_string(&value)
            .expect("serialization must succeed");

        let decoded: Value = serde_saphyr::from_str(&yaml)
            .unwrap_or_else(|err| {
                panic!(
                    "serializer emitted YAML that cannot be parsed:\n\
                     {yaml}\n\
                     error: {err}"
                )
            });

        prop_assert_eq!(decoded, value);
    }
}
