// U3C3: Spec Example 6.16. "TAG" directive
// %TAG !yaml! tag:yaml.org,2002: then using !yaml!str "foo"
// Expected result per suite: plain string "foo".

#[test]
fn yaml_u3c3_tag_directive() {
    let y = "%TAG !yaml! tag:yaml.org,2002:\n---\n!yaml!str \"foo\"\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse U3C3");
    assert_eq!(s, "foo");
}
