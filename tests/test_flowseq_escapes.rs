use serde_saphyr::FlowSeq;

// Repro for bug report and extended coverage:
// Input strings that look like YAML syntax/keywords or contain flow-structural characters
// must be quoted (and escaped as needed) in flow style so that round-trip preserves strings.
#[test]
fn flowseq_strings_are_quoted_when_needed() -> anyhow::Result<()> {
    let samples: Vec<&str> = vec![
        // structural characters and punctuation that break flow parsing
        "a, [], b",
        "{",
        "}",
        "[",
        "]",
        ",",
        "#",
        ":",
        "@",
        "`",
        // values that resemble booleans/null (YAML 1.2 + common 1.1 aliases)
        "n",
        "true",
        "False",
        "~",
        "null",
        "YES",
        "no",
        "off",
        "On",
        // numeric-looking tokens
        "100",
        "1e3",
        "3.14",
        ".nan",
        "-.inf",
        // leading whitespace should force quoting
        " leading",
        // embedded quotes, backslashes and control characters must be escaped
        "He said \"hi\"",
        "C:\\path",
        "line1\nline2",
        "\tindent",
        // single-char punctuation with special single-quote style in this serializer
        ".",
        "-",
    ];

    let yaml = serde_saphyr::to_string(&FlowSeq(&samples))?;

    // Expect all elements to be appropriately quoted/escaped in a flow sequence, with trailing newline.
    // Single quotes are preferred when no escaping is needed; double quotes are used for escapes.
    let serialized = concat!(
        r##"['a, [], b', '{', '}', '[', ']', ',', '#', ':', '@', '`', "##,
        r##"'n', 'true', 'False', '~', 'null', 'YES', 'no', 'off', 'On', '100', '1e3', '3.14', "##,
        r##"'.nan', '-.inf', ' leading', He said "hi", C:\path, "line1\nline2", "##,
        r##""\tindent", '.', '-']"##,
        "\n"
    );

    assert_eq!(yaml, serialized, "Unexpected YAML output: {yaml}");

    let deserialized: Vec<String> = serde_saphyr::from_str(&serialized)?;
    assert_eq!(samples, deserialized);

    Ok(())
}
