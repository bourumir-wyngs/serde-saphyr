use serde_saphyr::value::Tag;

#[test]
fn tag_equality_ignores_leading_bang() {
    let tag_plain = Tag::new("Thing").unwrap();
    let tag_banged = Tag::new("!Thing").unwrap();
    assert_eq!(tag_plain, tag_banged);
}

#[test]
fn tag_display_includes_bang() {
    let tag = Tag::new("Thing").unwrap();
    assert_eq!(format!("{}", tag), "!Thing");
}
