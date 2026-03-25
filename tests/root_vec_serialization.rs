#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Root {
    nested: Nested,
    top_level_list: Vec<u32>,
    after: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Nested {
    nested_list: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_vec_serialization() {
        let root = Root {
            nested: Nested { nested_list: vec![] },
            top_level_list: vec![],
            after: 1,
        };
        let serialized = serde_saphyr::to_string(&root).unwrap();
        println!("{serialized}");
        // nested:
        //   nested_list: []   <- correct
        // top_level_list:
        // []                  <- BUG: should be `top_level_list: []`
        // after: 1

        let deserialized: Root = serde_saphyr::from_str(&serialized).unwrap();
        assert_eq!(root, deserialized);
    }
}