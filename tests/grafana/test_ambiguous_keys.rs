use serde_saphyr::SerializerOptions;
use std::collections::HashMap;

#[test]
fn test_quote_ambiguous_keys() {
    let options = SerializerOptions {
        ..Default::default()
    };

    let mut map = HashMap::new();
    map.insert("y", 0);
    map.insert("n", 1);
    map.insert("yes", 2);
    map.insert("no", 3);
    map.insert("x", 4);
    map.insert("0", 4);

    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &map, options).unwrap();

    assert!(output.contains("\"y\":"), "y should be quoted");
    assert!(output.contains("\"n\":"), "n should be quoted");
    assert!(output.contains("\"yes\":"), "yes should be quoted");
    assert!(output.contains("\"no\":"), "no should be quoted");
    assert!(
        output.contains("\"0\":"),
        "number that is a string in key position should be quoted"
    );
    assert!(output.contains("x:"), "x should NOT be quoted");
}
