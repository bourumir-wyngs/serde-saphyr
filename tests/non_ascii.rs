use serde_json::{json, Value};

#[test]
fn test_non_ascii_comment_start() {
    let yaml = "\
# A \u{AC00}
a1:
  b: \u{AC00}
a2:
  b: \u{AC00}
";

    let v: Value = serde_saphyr::from_str(yaml).unwrap();

    let expected = json!({
        "a1": { "b": "\u{AC00}" },
        "a2": { "b": "\u{AC00}" }
    });

    assert_eq!(v, expected);
}

#[test]
fn test_non_ascii_comment_many() {
    let yaml = "\
# A \u{AC00}
\u{AC00}1: # A \u{AC00}
  b: 1 # A \u{AC00}
a2: # A \u{AC00} # A \u{AC00}
  b: \u{AC00} # A \u{AC00}
  c: [ 1, 2, 3 ] # \u{AC00}
  d: # \u{AC00}
    - 1 \u{AC00}
    - 2 \u{AC00}
    - 3 \u{AC00}
# A \u{AC00}
";

    let v: Value = serde_saphyr::from_str(yaml).unwrap();

    let expected = json!({
        "\u{AC00}1": { "b": 1 },
        "a2": {
            "b": "\u{AC00}",
            "c": [1, 2, 3],
            "d": ["1 \u{AC00}", "2 \u{AC00}", "3 \u{AC00}"]
        }
    });

    assert_eq!(v, expected);
}

#[test]
fn test_non_ascii_comment() {
    let yaml = "\
a1:
  b: 1
# A \u{AC00}
a2\u{AC00}:
  b: \u{AC00}
";

    let v: Value = serde_saphyr::from_str(yaml).unwrap();

    let expected = json!({
        "a1": { "b": 1 },
        "a2\u{AC00}": { "b": "\u{AC00}" }
    });

    assert_eq!(v, expected);
}
