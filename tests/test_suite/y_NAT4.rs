use serde::Deserialize;

// NAT4: Various empty or newline-only quoted strings
#[derive(Debug, Deserialize)]
struct Root {
    a: String,
    b: String,
    c: String,
    d: String,
    e: String,
    f: String,
    g: String,
    h: String,
}

#[test]
fn yaml_nat4_various_empty_or_newline_only_quoted_strings() {
    // Notes:
    // - Quoted scalars preserve spaces exactly.
    // - In quoted scalars, a single line break between non-empty lines folds to a space,
    //   but *empty lines* are preserved as '\n'.
    // - Continuation lines of a multi-line quoted scalar must be indented ≥1 space.
    let y = r#"---
a: ' '
b: '  '
c: " "
d: "  "
e: '

 '
f: "\n"
g: '


 '
h: "\n\n"
"#;

    let v: Root = serde_saphyr::from_str(y).expect("failed to parse NAT4");

    // Single- and double-quoted one-space
    assert_eq!(v.a, " ");
    assert_eq!(v.c, " ");

    // Two spaces preserved exactly
    assert_eq!(v.b, "  ");
    assert_eq!(v.d, "  ");

    // One empty line inside single-quoted => "\n"
    assert_eq!(v.e, "\n");

    // Double-quoted escape => "\n"
    assert_eq!(v.f, "\n");

    // Two empty lines inside single-quoted => "\n\n"
    assert_eq!(v.g, "\n\n");

    // Double-quoted with two escapes => "\n\n"
    assert_eq!(v.h, "\n\n");
}
