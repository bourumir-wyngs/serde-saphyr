// Extended cases that require typed, event, or rejection assertions.

use super::yaml_suite_support::{assert_invalid_case, assert_json_case, assert_valid_events};
use serde::Deserialize;

#[test]
fn yaml_33x3_explicit_integer_tags() {
    let yaml = "---\n- !!int 1\n- !!int -2\n- !!int 33\n";
    let actual: Vec<i32> = serde_saphyr::from_str(yaml).expect("33X3 must deserialize");
    assert_eq!(actual, vec![1, -2, 33]);
}

#[derive(Debug, Deserialize, PartialEq)]
struct TaggedMapping {
    a: String,
    c: i32,
    e: String,
    g: String,
    #[serde(rename = "23")]
    twenty_three: bool,
}

#[test]
fn yaml_74h7_explicit_tags_on_mapping_entries() {
    let yaml = "!!str a: b\nc: !!int 42\ne: !!str f\ng: h\n!!str 23: !!bool false\n";
    let actual: TaggedMapping = serde_saphyr::from_str(yaml).expect("74H7 must deserialize");
    let expected = TaggedMapping {
        a: "b".to_owned(),
        c: 42,
        e: "f".to_owned(),
        g: "h".to_owned(),
        twenty_three: false,
    };
    assert_eq!(actual, expected);
}

#[test]
fn yaml_f2c7_anchors_and_explicit_tags_in_sequence() {
    let yaml = " - &a !!str a\n - !!int 2\n - !!int &c 4\n - &d d\n";
    let actual: (String, i32, i32, String) =
        serde_saphyr::from_str(yaml).expect("F2C7 must deserialize");
    assert_eq!(actual, ("a".to_owned(), 2, 4, "d".to_owned()));
}

#[test]
fn yaml_dff7_flow_mapping_empty_key_and_value_events() {
    let yaml = "{\n? explicit: entry,\nimplicit: entry,\n?\n}\n";
    let events = "+STR\n+DOC\n+MAP {}\n=VAL :explicit\n=VAL :entry\n=VAL :implicit\n=VAL :entry\n=VAL :\n=VAL :\n-MAP\n-DOC\n-STR\n";
    assert_valid_events(yaml, events);
}

#[test]
fn yaml_fh7j_explicit_empty_tagged_scalars_events() {
    let yaml = "- !!str\n-\n  !!null : a\n  b: !!str\n- !!str : !!null\n";
    let events = "+STR\n+DOC\n+SEQ\n=VAL <tag:yaml.org,2002:str> :\n+MAP\n=VAL <tag:yaml.org,2002:null> :\n=VAL :a\n=VAL :b\n=VAL <tag:yaml.org,2002:str> :\n-MAP\n+MAP\n=VAL <tag:yaml.org,2002:str> :\n=VAL <tag:yaml.org,2002:null> :\n-MAP\n-SEQ\n-DOC\n-STR\n";
    assert_valid_events(yaml, events);
}

#[test]
fn yaml_frk4_flow_mapping_explicit_empty_entries_events() {
    let yaml = "{\n  ? foo :,\n  : bar,\n}\n";
    let events = "+STR\n+DOC\n+MAP {}\n=VAL :foo\n=VAL :\n=VAL :\n=VAL :bar\n-MAP\n-DOC\n-STR\n";
    assert_valid_events(yaml, events);
}

#[test]
fn yaml_kk5p_complex_keys_events() {
    let yaml = "complex1:\n  ? - a\ncomplex2:\n  ? - a\n  : b\ncomplex3:\n  ? - a\n  : >\n    b\ncomplex4:\n  ? >\n    a\n  :\ncomplex5:\n  ? - a\n  : - b\n";
    let events = "+STR\n+DOC\n+MAP\n=VAL :complex1\n+MAP\n+SEQ\n=VAL :a\n-SEQ\n=VAL :\n-MAP\n=VAL :complex2\n+MAP\n+SEQ\n=VAL :a\n-SEQ\n=VAL :b\n-MAP\n=VAL :complex3\n+MAP\n+SEQ\n=VAL :a\n-SEQ\n=VAL >b\\n\n-MAP\n=VAL :complex4\n+MAP\n=VAL >a\\n\n=VAL :\n-MAP\n=VAL :complex5\n+MAP\n+SEQ\n=VAL :a\n-SEQ\n+SEQ\n=VAL :b\n-SEQ\n-MAP\n-MAP\n-DOC\n-STR\n";
    assert_valid_events(yaml, events);
}

#[test]
fn yaml_55wf_invalid_escape() {
    assert_invalid_case("---\n\"\\.\"\n");
}

#[test]
fn yaml_62ez_flow_mapping_followed_by_unseparated_scalar() {
    assert_invalid_case("---\nx: { y: z }in: valid\n");
}

#[test]
fn yaml_8xdj_invalid_plain_scalar_continuation() {
    assert_invalid_case("key: word1\n#  xxx\n  word2\n");
}

#[test]
fn yaml_9jba_comment_without_separation_after_flow_sequence() {
    assert_invalid_case("---\n[ a, b, c, ]#invalid\n");
}

#[test]
fn yaml_cvw2_comment_without_separation_in_flow_sequence() {
    assert_invalid_case("---\n[ a, b, c,#invalid\n]\n");
}

#[test]
fn yaml_cxx2_anchor_on_document_start() {
    assert_invalid_case("--- &anchor a: b\n");
}

#[test]
fn yaml_d49q_multiline_single_quoted_implicit_key() {
    assert_invalid_case("'a\\nb': 1\n'c\n d': 1\n");
}

#[test]
fn yaml_dk4h_multiline_flow_key() {
    assert_invalid_case("---\n[ key\n  : value ]\n");
}

#[test]
fn yaml_dk95_01_tab_in_double_quoted_continuation() {
    assert_invalid_case("foo: \"bar\n\tbaz\"\n");
}

#[test]
fn yaml_dk95_06_tab_indented_mapping_entry() {
    assert_invalid_case("foo:\n  a: 1\n  \tb: 2\n");
}

#[test]
fn yaml_dmg6_inconsistent_mapping_indentation() {
    assert_invalid_case("key:\n  ok: 1\n wrong: 2\n");
}

#[test]
fn yaml_eb22_directive_without_document_end() {
    assert_invalid_case("---\nscalar1 # comment\n%YAML 1.2\n---\nscalar2\n");
}

#[test]
fn yaml_g5u8_bare_indicators_in_flow_sequence() {
    assert_invalid_case("---\n- [-, -]\n");
}

#[test]
fn yaml_g7je_multiline_plain_implicit_key() {
    assert_invalid_case("a\\nb: 1\nc\n d: 1\n");
}

#[test]
fn yaml_g9hc_anchor_before_zero_indented_sequence() {
    assert_invalid_case("---\nseq:\n&anchor\n- a\n- b\n");
}

#[test]
fn yaml_gdy7_unseparated_top_level_plain_scalars() {
    assert_invalid_case("key: value\nthis is #not a: key\n");
}

#[test]
fn yaml_gt5m_anchor_between_sequence_entries() {
    assert_invalid_case("- item1\n&node\n- item2\n");
}

#[test]
fn yaml_h7j7_tag_after_anchor_on_separate_line() {
    assert_invalid_case("key: &x\n!!map\n  a: b\n");
}

#[test]
fn yaml_h7tq_yaml_directive_with_extra_argument() {
    assert_invalid_case("%YAML 1.2 foo\n---\n");
}

#[test]
fn yaml_hre5_invalid_single_quote_escape_in_double_quotes() {
    assert_invalid_case("---\ndouble: \"quoted \\' scalar\"\n");
}

#[test]
fn yaml_hu3p_mapping_nested_under_plain_scalar() {
    assert_invalid_case("key:\n  word1 word2\n  no: key\n");
}

#[test]
fn yaml_jy7z_unseparated_mapping_after_quoted_value() {
    assert_invalid_case(
        "key1: \"quoted1\"\nkey2: \"quoted2\" no key: nor value\nkey3: \"quoted3\"\n",
    );
}

#[test]
fn yaml_ks4u_scalar_after_flow_sequence() {
    assert_invalid_case("---\n[\nsequence item\n]\ninvalid item\n");
}

#[test]
fn yaml_u99r_tag_without_node_before_flow_comma() {
    assert_invalid_case("- !!str, xxx\n");
}

#[test]
fn yaml_dk3j_zero_indented_root_folded_scalar() {
    let yaml = "--- >\nline1\n# no comment\nline3\n";
    let expected_json = "\"line1 # no comment line3\\n\"\n";
    assert_json_case(yaml, expected_json);
}

#[test]
fn yaml_fp8r_zero_indented_root_folded_scalar() {
    let yaml = "--- >\nline1\nline2\nline3\n";
    let expected_json = "\"line1 line2 line3\\n\"\n";
    assert_json_case(yaml, expected_json);
}
