use serde_saphyr::SerializerOptions;

/// Test that long multiline strings are emitted as literal block scalars when enabled
#[test]
fn test_quoted_multiline_wrap_with_block_scalars_enabled() {
    let options = SerializerOptions {
        indent_step: 2,
        prefer_block_scalars: true,
        ..Default::default()
    };

    let httpd_segment = r#" Deny access to the entirety of your server's filesystem. You must
# explicitly permit access to web content directories in other
# <Directory> blocks below.
#
<Directory />
    AllowOverride none
    Require all denied
</Directory>

#
# Note that from this point forward you must specifically allow
# particular features to be enabled"#;

    let data = serde_json::json!({ "httpd.conf": httpd_segment });

    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // Multiline + long: emit as literal block scalar for readability.
    assert!(
        output.starts_with("httpd.conf: |"),
        "Should use literal block scalar for long multiline content.\nOutput:\n{}",
        output
    );

    // Verify lines are reasonably sized (not all on one line)
    // Go's yaml.v2 algorithm breaks at spaces only when column > best_width (80).
    // If there's no space before column 80, the line continues until a break point.
    let max_line_len = output.lines().map(|l| l.len()).max().unwrap_or(0);
    assert!(
        max_line_len <= 120,
        "Block scalar should keep reasonable line widths; max was {}.\nOutput:\n{}",
        max_line_len,
        output
    );

    // Round-trip must preserve exact content
    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(
        parsed["httpd.conf"].as_str().unwrap(),
        httpd_segment,
        "Round-trip failed!"
    );
}

/// Test that line folding preserves leading spaces using backslash escape
#[test]
fn test_quoted_wrap_preserves_leading_spaces() {
    let options = SerializerOptions {
        indent_step: 2,
        
        prefer_block_scalars: false,
        ..Default::default()
    };

    // Content with 4-space indents after newlines
    let value = "line1\n    indented line2\n    indented line3";

    let data = serde_json::json!({ "key": value });

    let mut output = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

    // Round-trip must preserve the 4-space indents
    let parsed: serde_json::Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(
        parsed["key"].as_str().unwrap(),
        value,
        "Round-trip must preserve indented content.\nOutput:\n{}",
        output
    );
}

