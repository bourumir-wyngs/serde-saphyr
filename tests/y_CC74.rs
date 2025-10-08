// CC74: Tag Handles — %TAG with local handle; scalar should deserialize to "bar"
#[test]
fn yaml_cc74_tag_handles_scalar() {
    let y = "%TAG !e! tag:example.com,2000:app/\n---\n!e!foo \"bar\"\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse CC74");
    assert_eq!(s, "bar");
}
