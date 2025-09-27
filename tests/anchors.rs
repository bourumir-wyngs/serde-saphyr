use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Root {
    seq: Vec<Vec<i32>>,
}

/// Parses a YAML string into `Root`.
///
/// # Parameters
/// - `y`: YAML document as UTF-8 text.
///
/// # Returns
/// - `Root` struct with `seq: Vec<Vec<i32>>`.
fn parse_yaml(y: &str) -> Root {
    serde_saphyr::from_str::<Root>(y).expect("valid YAML that matches `Root`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yaml_with_anchors() {
        let y = "\
seq:
  - &A [1,2,3]
  - *A
  - *A
  - *A
  - *A
";

        let mut data = parse_yaml(y);

        // Basic shape checks
        assert_eq!(data.seq.len(), 5);
        for v in &data.seq {
            assert_eq!(v, &vec![1, 2, 3]);
        }

        // Prove aliases are deserialized as independent vectors (not shared backing storage)
        data.seq[0][0] = 999;
        assert_eq!(data.seq[0], vec![999, 2, 3]);
        // Others stay unchanged
        for v in &data.seq[1..] {
            assert_eq!(v, &vec![1, 2, 3]);
        }
    }
}
