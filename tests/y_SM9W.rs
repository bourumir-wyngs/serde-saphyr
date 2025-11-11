use std::collections::HashMap;
#[test]
fn y_sm9w_single_dash() {
    // Just dash followed by new line is a single entry list holding null.
    let y1 = "-\n";
    let r1: Vec<Option<String>> = serde_saphyr::from_str(y1)
        .expect("Parser failed to handle single dash as a sequence entry");

    assert_eq!(1, r1.len());
    assert!(r1[0].is_none());
}

#[test]
fn y_sm9w_single_colon() {
    // Just colon followed by new line is the mapping of null key to null value
    let y2 = ":\n";
    let r2: HashMap<Option<String>, Option<String>> =
        serde_saphyr::from_str(y2).expect("Parser failed to handle single colon");

    assert_eq!(r2.len(), 1);
    // key is None, value is None
    assert!(r2.contains_key(&None::<String>));
    assert_eq!(r2.get(&None::<String>), Some(&None));
}
