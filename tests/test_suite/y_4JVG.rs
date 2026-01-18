// 4JVG: Scalar value with two anchors — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_4jvg_scalar_with_two_anchors_should_fail() {
    let y = "top1: &node1\n  &k1 key1: val1\ntop2: &node2\n  &v2 val2\n";
    use std::collections::HashMap;
    let result: Result<HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "4JVG should fail due to invalid double-anchored scalar context"
    );
}
