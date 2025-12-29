use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
enum Key {
    Str(String),
    Seq(Vec<String>),
}

#[test]
fn y_rzp5() {
    // Inline YAML from test-suite case RZP5: Various Trailing Comments [1.3]
    let yaml = r#"a: "double
  quotes" # lala
b: plain
 value  # lala
c  : #lala
  d
? # lala
 - seq1
: # lala
 - #lala
  seq2
e: &node # lala
 - x: y
block: > # lala
  abcde
"#;

    // We parse into a BTreeMap with a flexible Key that can be either a String or a sequence of strings,
    // and serde_json::Value for values (sufficient for verifying shapes and leaf scalars here).
    let map: BTreeMap<Key, serde_json::Value> =
        serde_saphyr::from_str(yaml).expect("parse YAML with structured key");

    // a: "double quotes"
    assert_eq!(
        map.get(&Key::Str("a".into()))
            .and_then(|v| v.as_str())
            .unwrap(),
        "double quotes"
    );

    // b: plain value
    assert_eq!(
        map.get(&Key::Str("b".into()))
            .and_then(|v| v.as_str())
            .unwrap(),
        "plain value"
    );

    // c: d
    assert_eq!(
        map.get(&Key::Str("c".into()))
            .and_then(|v| v.as_str())
            .unwrap(),
        "d"
    );

    // ? - seq1 : - seq2  => key is a one-element sequence ["seq1"], value is sequence ["seq2"]
    let k = Key::Seq(vec!["seq1".into()]);
    let v = map.get(&k).expect("sequence key present");
    let arr = v
        .as_array()
        .expect("value under sequence key is a sequence/array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].as_str().unwrap(), "seq2");

    // e: &node - x: y   => value is a sequence with a mapping {x: y}
    let e_val = map.get(&Key::Str("e".into())).expect("key e present");
    let e_seq = e_val.as_array().expect("e is a sequence");
    assert_eq!(e_seq.len(), 1);
    let e_map = e_seq[0].as_object().expect("first element is a mapping");
    assert_eq!(e_map.len(), 1);
    let only_val = e_map.values().next().expect("single value present");
    let ok = only_val.as_str() == Some("y") || only_val.as_bool() == Some(true);
    assert!(
        ok,
        "expected value under 'x' to be 'y' (string) or true (bool), got: {:?}",
        only_val
    );

    // block: > # lala  abcde  (folded block scalar) — by common YAML rules this becomes "abcde\n"
    let block = map
        .get(&Key::Str("block".into()))
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(block, "abcde\n");
}
