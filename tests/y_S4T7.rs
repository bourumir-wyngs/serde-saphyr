use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Doc { aaa: String }

// S4T7: Document with footer ("...")
#[test]
fn yaml_s4t7_document_with_footer() {
    let y = "aaa: bbb\n...\n";
    let d: Doc = serde_saphyr::from_str(y).unwrap();
    assert_eq!(d, Doc { aaa: "bbb".into() });
}
