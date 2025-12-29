use indoc::indoc;
use {serde_saphyr::*, std::collections::*};

#[test]
pub fn empty_maps() {
    let mut map1 = HashMap::new();
    map1.insert("key1", "value1");
    let mut map2 = HashMap::new();
    map2.insert("key2", map1);
    let mut map3 = HashMap::new();
    map3.insert("key3", map2);
    let mut map4 = HashMap::new();
    map4.insert("key4", map3);

    let mut string = String::default();
    to_fmt_writer(&mut string, &map4).unwrap();
    print!("{}", string);
    let expected = indoc! {r#"
key4:
  key3:
    key2:
      key1: value1
"#};
    assert_eq!(string, expected);
}
