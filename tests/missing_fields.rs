use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Item {
    name: String,
    platform: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Root {
    item: Item,
}

fn assert_has_snippet(rendered: &str) {
    assert!(
        rendered.contains(" -->"),
        "expected snippet header, got: {rendered}"
    );
    assert!(
        rendered.contains('^'),
        "expected span marker in snippet, got: {rendered}"
    );
}

#[test]
fn missing_required_field_in_nested_struct_renders_snippet_at_parent_container() {
    let yaml = concat!("item:\n", "  name: test\n");

    let err = serde_saphyr::from_str::<Root>(yaml).expect_err("must fail");
    let rendered = err.to_string();
    // println!("{rendered}");

    assert!(
        rendered.contains("missing field `platform`"),
        "expected missing-field message, got: {rendered}"
    );
    assert_has_snippet(&rendered);
}

#[test]
fn missing_required_field_in_sequence_item_renders_snippet_at_item_container() {
    let yaml = "- name: test\n";

    let err = serde_saphyr::from_str::<Vec<Item>>(yaml).expect_err("must fail");
    let rendered = err.to_string();
    // println!("{rendered}");

    assert!(
        rendered.contains("missing field `platform`"),
        "expected missing-field message, got: {rendered}"
    );
    assert_has_snippet(&rendered);
}
