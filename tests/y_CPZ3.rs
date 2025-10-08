use serde::Deserialize;

// CPZ3: Doublequoted scalar starting with a tab
#[derive(Debug, Deserialize)]
struct Doc { tab: String }

#[test]
fn yaml_cpz3_doublequoted_scalar_starting_with_tab() {
    let y = "---\ntab: \"\tstring\"\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse CPZ3");
    assert_eq!(d.tab, "\tstring");
}
