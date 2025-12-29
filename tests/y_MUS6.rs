use serde::Deserialize;

// MUS6: Directive variants
// We add separate tests per case in the YAML suite. Where the suite marks fail: true,
// we assert that parsing returns an error. For valid/unknown directives followed by an
// empty document (---), we assert deserializing Option<String> yields None.

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy(String);

// Case 1: "%YAML 1.1#..." — invalid directive syntax → should error.
#[test]
fn yaml_mus6_invalid_yaml_1_1_with_trailing() {
    let y = "%YAML 1.1#...\n---\n";
    let r: Result<Dummy, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "MUS6 case1 should fail to parse");
}

// Case 2: two %YAML 1.2 directives in one stream → suite marks fail:true
#[test]
fn yaml_mus6_two_yaml_1_2_directives_should_fail() {
    let y = "%YAML 1.2\n---\n%YAML 1.2\n---\n";
    let r: Result<Vec<serde::de::IgnoredAny>, _> = serde_saphyr::from_multiple(y);
    assert!(
        r.is_err(),
        "MUS6 case2 should fail to parse due to repeated directives"
    );
}

// Case 3: "%YAML  1.1" — extra space, then an empty doc. Expect None.
#[test]
fn yaml_mus6_yaml_with_double_space_then_empty_doc() {
    let y = "%YAML  1.1\n---\n";
    let v: Option<String> = serde_saphyr::from_str(y).expect("MUS6 case3 parse failed");
    assert_eq!(v, None);
}

// Case 4: "%YAML 1.1  # comment" — directive with comment, then empty doc → None.
#[test]
fn yaml_mus6_yaml_1_1_with_comment_then_empty_doc() {
    let y = "%YAML 1.1  # comment\n---\n";
    let v: Option<String> = serde_saphyr::from_str(y).expect("MUS6 case4 parse failed");
    assert_eq!(v, None);
}

// Case 5: Reserved/unknown directives should be ignored by parser. Empty doc follows.
#[test]
fn yaml_mus6_reserved_directive_yam_then_empty_doc() {
    let y = "%YAM 1.1\n---\n"; // reserved directive name
    let v: Option<String> = serde_saphyr::from_str(y).expect("MUS6 case5 parse failed");
    assert_eq!(v, None);
}

#[test]
fn yaml_mus6_reserved_directive_yamll_then_empty_doc() {
    let y = "%YAMLL 1.1\n---\n"; // reserved directive name
    let v: Option<String> = serde_saphyr::from_str(y).expect("MUS6 case5b parse failed");
    assert_eq!(v, None);
}

// Case 6: "%YAML \t 1.1" — directive with TABs around the version.
#[test]
fn yaml_mus6_yaml_with_tab_between_name_and_version() {
    let y = "%YAML \t 1.1\n---\n";
    let v: Option<String> = serde_saphyr::from_str(y).expect("MUS6 case6 parse failed");
    // Expect an empty document
    assert_eq!(v, None);
}
