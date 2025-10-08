use serde::Deserialize;

// 5BVJ: Block Scalar Indicators
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    literal: String,
    folded: String,
}

#[test]
fn yaml_5bvj_block_scalar_indicators() {
    let y = "literal: |\n  some\n  text\nfolded: >\n  some\n  text\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse 5BVJ");
    assert_eq!(d.literal, "some\ntext\n");
    assert_eq!(d.folded, "some text\n");
}
