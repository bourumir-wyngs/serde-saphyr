use serde::Deserialize;

// UV7Q: Legal tab after indentation — multiline plain scalar folded into one value
// YAML intends that the second physical line belongs to the first sequence item, producing "x x".

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    x: Vec<String>,
}

#[test]
fn yaml_uv7q_tab_after_indentation() {
    // `   \tx` starts with 3 *spaces* (valid indentation). The tab is *after* indentation, so it’s allowed
    // as scalar whitespace. The indented line continues the plain scalar from `- x`, and plain scalars fold
    // the newline to a space => `"x x"`.
    let yaml = "x:\n - x\n   \tx\n";

    let v: Doc = serde_saphyr::from_str(yaml).expect("failed to parse UV7Q");
    assert_eq!(v.x, vec!["x x".to_string()]);
}
