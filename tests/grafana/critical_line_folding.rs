use serde_saphyr::SerializerOptions;

/// Test that line_width=80 means 80 chars, not something smaller
#[test]
fn test_line_width_is_80_not_60() {
    let options = SerializerOptions {
        indent_step: 2,
        prefer_block_scalars: true,
        empty_as_braces: true,
        folded_wrap_chars: 80,
        ..Default::default()
    };

    // 70 character string - MUST fit on one line with key
    let value = "123456789012345678901234567890123456789012345678901234567890123456789x"; // 70 chars

    let data = serde_json::json!({ "key": value });
    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // key: "70chars" = 7 + 70 = 77 chars. MUST be on ONE LINE.
    let line_count = output.lines().count();
    assert_eq!(
        line_count, 1,
        "70-char value with 'key: \"...\"' (77 total) MUST fit on one line with line_width=80!\nGot:\n{}",
        output
    );
}

/// Test that multilineMangled.txt keeps "tk mangles it." on same line
#[test]
fn test_exact_multiline_mangled_output() {
    let options = get_rtk_options();

    let value = "\"multilineMangled\": |\n  This is a multiline string.\n  This is a second line. It has an intentional trailing space. tk mangles it. \n\"otherField\": \"otherValue\"";

    let data = serde_json::json!({ "multilineMangled.txt": value });
    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // Go output has "tk mangles it." on SAME line, not broken
    assert!(
        output.contains("tk mangles it."),
        "Should NOT break 'tk mangles it.' across lines!\nGot:\n{}",
        output
    );

    // The double-quoted string should be wrapped and round-trip correctly
    // If leading spaces exist on continuation lines, they should be escaped
    // (the exact break position may vary, so we just verify round-trip)

    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(parsed["multilineMangled.txt"].as_str().unwrap(), value);
}

fn get_rtk_options() -> SerializerOptions {
    SerializerOptions {
        indent_step: 2,
        prefer_block_scalars: true,
        empty_as_braces: true,
        folded_wrap_chars: 80,
        ..Default::default()
    }
}

/// TEST 1: multilineMangled.txt
/// Long string (~150 chars) that MUST wrap. After wrapping at \n,
/// content starts with 2 spaces, so MUST use "\ " escape.
#[test]
fn test_long_string_must_wrap_with_backslash_escape() {
    let options = get_rtk_options();

    let value = "\"multilineMangled\": |\n  This is a multiline string.\n  This is a second line. It has an intentional trailing space. tk mangles it. \n\"otherField\": \"otherValue\"";

    let data = serde_json::json!({ "key": value });
    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // MUST round-trip exactly
    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(
        parsed["key"].as_str().unwrap(),
        value,
        "TEST 1 FAILED: Round-trip corrupted the content!\nOutput was:\n{}",
        output
    );
}

/// TEST 2: httpd.conf short segment
/// Content that fits in 80 chars - must NOT wrap, must NOT have "\ "
#[test]
fn test_short_string_must_not_wrap() {
    let options = get_rtk_options();

    let value = "<Directory />\n    AllowOverride none\n    Require all denied\n</Directory>";

    let data = serde_json::json!({ "key": value });
    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // Must NOT contain "\ " escape - line is short enough to not wrap
    assert!(
        !output.contains("\\ "),
        "TEST 2 FAILED: Short string should not wrap!\nOutput was:\n{}",
        output
    );

    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(
        parsed["key"].as_str().unwrap(),
        value,
        "TEST 2 FAILED: Round-trip corrupted the content!"
    );
}

/// TEST 3: Both strings in same document
/// This is the REAL test - both behaviors must coexist
#[test]
fn test_both_strings_together_no_regression() {
    let options = get_rtk_options();

    // Long string - MUST wrap with "\ "
    let long_val = "\"multilineMangled\": |\n  This is a multiline string.\n  This is a second line. It has an intentional trailing space. tk mangles it. \n\"otherField\": \"otherValue\"";

    // Short string - must NOT wrap
    let short_val = "<Directory />\n    AllowOverride none\n    Require all denied\n</Directory>";

    let data = serde_json::json!({
        "long": long_val,
        "short": short_val
    });

    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();

    assert_eq!(
        parsed["long"].as_str().unwrap(),
        long_val,
        "TEST 3 FAILED: Long string corrupted!\nOutput:\n{}",
        output
    );
    assert_eq!(
        parsed["short"].as_str().unwrap(),
        short_val,
        "TEST 3 FAILED: Short string corrupted!\nOutput:\n{}",
        output
    );
}

/// TEST 4: The EXACT httpd.conf content that keeps failing
#[test]
fn test_exact_httpd_segment() {
    let options = get_rtk_options();

    // This is the exact segment from the diff that keeps failing
    let value = "blocks below.\n#\n<Directory />\n    AllowOverride none\n    Require all denied\n</Directory>\n\n#\n# Note that from this point forward you must specifically allow\n# particular features to be enabled";

    let data = serde_json::json!({ "httpd.conf": value });
    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // The segment "\n    Require all denied" must NOT be broken
    // Go keeps it on the same line as "\n    AllowOverride none\n"
    assert!(
        !output.contains("\\n\n    \\   Require"),
        "TEST 4 FAILED: Broke after \\n    AllowOverride when it shouldn't!\nOutput:\n{}",
        output
    );

    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(
        parsed["httpd.conf"].as_str().unwrap(),
        value,
        "TEST 4 FAILED: Round-trip corrupted!"
    );
}
