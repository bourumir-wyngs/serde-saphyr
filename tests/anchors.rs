#[cfg(test)]
mod tests {

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

    // ---------------------------------------------------------------------
    // Serialization tests demonstrating README's anchor wrappers (Rc, Weak)
    // ---------------------------------------------------------------------
    use indoc::indoc;
    use serde::Serialize;
    use std::rc::Rc;
    use serde_saphyr::{to_string, RcAnchor, RcWeakAnchor};

    #[derive(Clone, Serialize)]
    struct Node {
        name: String,
    }

    #[test]
    fn rc_anchor_shared_in_sequence_produces_anchor_and_alias() {
        let shared = Rc::new(Node { name: "node one".into() });
        let data: Vec<RcAnchor<Node>> = vec![RcAnchor(shared.clone()), RcAnchor(shared)];
        let yaml = to_string(&data).expect("serialize RcAnchor sequence");

        let expected = indoc! {r#"
            - &a1
              name: node one
            - *a1
        "#};
        assert_eq!(yaml, expected, "RcAnchor seq YAML mismatch. Got:\n{}", yaml);
    }

    #[test]
    fn rc_weak_anchor_present_and_dangling() {
        // Present (upgraded) weak -> emits anchored value
        let strong = Rc::new(Node { name: "strong".into() });
        let weak_present = Rc::downgrade(&strong);
        let yaml_present = to_string(&RcWeakAnchor(weak_present)).expect("serialize RcWeakAnchor present");
        let expected_present = indoc! {r#"
            &a1
            name: strong
        "#};
        assert_eq!(yaml_present, expected_present, "RcWeakAnchor (present) YAML mismatch. Got:\n{}", yaml_present);

        // Dangling weak -> null
        let weak_dangling = {
            let tmp = Rc::new(Node { name: "tmp".into() });
            Rc::downgrade(&tmp)
        }; // tmp dropped here
        let yaml_null = to_string(&RcWeakAnchor(weak_dangling)).expect("serialize RcWeakAnchor dangling");
        assert_eq!(yaml_null, "null\n");
    }
}
