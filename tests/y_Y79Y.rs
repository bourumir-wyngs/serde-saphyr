use serde::Deserialize;

// Y79Y: Tabs in various contexts
// The expected mapping is { foo: "\t\n", bar: 1 }.

#[derive(Debug, Deserialize)]
struct Doc {
    foo: String,
    bar: i64,
}

// Known issue of serde-saphyr: while standard YAML only disallows tabs in indentation,
// saphyr rejects any tabs in unquoted scalars. This actually looks quite a sound
// idea so I am not even sure if it is worth pushing change to saphyr-parser.
#[test]
#[ignore]
fn yaml_y79y_block_scalar_with_tab() {
    // Use escape sequences for the TAB and newlines to avoid embedding a literal TAB in the file.
    // Note: Our parser currently errors when the first content line of a block scalar is a TAB.
    let y = "foo: |\n\t\nbar: 1\n";
    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse Y79Y (tabs in block scalar)");
    assert_eq!(v.foo, "\t\n");
    assert_eq!(v.bar, 1);
}

#[test]
fn yaml_y79y_quoted_scalar_with_tab() {
    let y = "foo: \"\\t\\n\"\nbar: 1\n";
    let v: Doc = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.foo, "\t\n");
    assert_eq!(v.bar, 1);
}