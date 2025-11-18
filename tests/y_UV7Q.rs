use serde::Deserialize;

// UV7Q: Legal tab after indentation — multiline plain scalar folded into one value
// YAML intends that the second physical line belongs to the first sequence item, producing "x x".

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    x: Vec<String>,
}

// The error is "while scanning a plain scalar, found a tab"
// Tabs are okay in scalar body
// Submitted https://github.com/saphyr-rs/saphyr/issues/89
#[ignore]
#[test]
fn yaml_uv7q_tab_after_indentation() {
    let y = "x:\n - x\n \tx\n";
    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse UV7Q");
    assert_eq!(v.x, vec!["x x".to_string()]);
}
