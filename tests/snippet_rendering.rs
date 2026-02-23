use serde::Deserialize;
use serde_saphyr::{from_str, from_str_with_options};

/// Specialized regression test for snippet rendering.
///
/// This asserts that the rendered error output includes actual source text from the
/// provided YAML input (not only the location / message).
#[test]
fn error_renders_snippet_text_when_available() {
    // Deterministic error with a known location (1:1) and a distinctive source line.
    let yaml = "*missing\n";

    let err = from_str::<String>(yaml).expect_err("unknown anchor should error");
    let rendered = err.to_string();

    // Location/title prefix from snippet rendering.
    assert!(
        rendered.contains("<input>:1:1"),
        "expected location prefix in snippet output, got:\n{rendered}"
    );

    // Original message must still be present.
    assert!(
        rendered.contains("unknown anchor"),
        "expected original error message to be present, got:\n{rendered}"
    );

    // The snippet must include the offending YAML text.
    assert!(
        rendered.contains("*missing"),
        "expected snippet to include original source line, got:\n{rendered}"
    );

    // Marker should be rustc-like caret (not a box-drawing underline).
    assert!(
        rendered.contains('^'),
        "expected caret marker in snippet output, got:\n{rendered}"
    );
    assert!(
        !rendered.contains('━'),
        "expected no box-drawing underline marker in snippet output, got:\n{rendered}"
    );
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Cfg {
    base_scalar: serde_saphyr::Spanned<u64>,
    key: Vec<usize>,
}

/// This mirrors the `examples/render_with_snipped.rs` sample as a regression test.
#[test]
fn example_render_with_snippet_is_covered_by_tests() {
    // Intentionally invalid YAML to demonstrate snippet rendering.
    // Move closing bracket under "key" to result a valid YAML.
    let yaml = r#"
    base_scalar: x123
    key: [ 1, 2, 2 ]
"#;

    let err = from_str::<Cfg>(yaml).expect_err("invalid integer should error");
    let rendered = err.to_string();

    // We expect snippet rendering (Options default with_snippet=true) and inclusion of source text.
    assert!(
        rendered.contains("<input>:")
            && rendered.contains("base_scalar")
            && rendered.contains("x123"),
        "expected snippet output to include location and source text, got:\n{rendered}"
    );
}

/// This mirrors the `examples/render_with_snipped.rs` sample as a regression test.
#[test]
fn example_render_with_snippet_is_covered_by_tests_2() {
    // Intentionally invalid YAML to demonstrate snippet rendering.
    // Move closing bracket under "key" to result a valid YAML.
    let yaml = r#"
    base_scalar: !!str x123
    key: [ 1, 2, 2 ]
"#;

    let err = from_str::<Cfg>(yaml).expect_err("invalid integer should error");
    let rendered = err.to_string();

    // We expect snippet rendering (Options default with_snippet=true) and inclusion of source text.
    assert!(
        rendered.contains("<input>:")
            && rendered.contains("base_scalar")
            && rendered.contains("x123"),
        "expected snippet output to include location and source text, got:\n{rendered}"
    );
}

#[test]
fn snippet_includes_two_lines_before_and_one_after_when_available() {
    // Error is on line 2; snippet should include line 1 (one of the two "before" lines)
    // and line 3 (the "after" line).
    let yaml = "ok: 1\nbad: *missing\nnext: 2\n";

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Doc {
        ok: i32,
        bad: String,
        next: i32,
    }

    let err = from_str::<Doc>(yaml).expect_err("unknown anchor should error");
    let rendered = err.to_string();

    assert!(
        rendered.contains("<input>:2:"),
        "expected location to point at line 2, got:\n{rendered}"
    );
    assert!(
        rendered.contains("ok: 1"),
        "expected snippet to include context line before error, got:\n{rendered}"
    );
    assert!(
        rendered.contains("bad: *missing"),
        "expected snippet to include error line, got:\n{rendered}"
    );
    assert!(
        rendered.contains("next: 2"),
        "expected snippet to include context line after error, got:\n{rendered}"
    );
}

#[test]
fn snippet_crops_very_long_lines_around_error_column() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Data {
        before: String,
        bad_bad_anchor_reference: String,
        after: String,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Doc {
        data: Data,
    }

    let yaml = r#"data:
    before_0: "@#$%^&*()_++_)(*&^%$#@!"
    before_1: "ZYXWVUTSRQPONMLKJIHGFEDCBA9876543210"
    before_2: "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    bad_bad_anchor_reference: *very_bad
    after_1: "0123456789abcdefghijklmnopqrstuvwxyz"
    after_2: "!@#$%^&*()_++_)(*&^%$#@"
"#;

    let opts = serde_saphyr::options! { crop_radius: 10 };

    let err = from_str_with_options::<Doc>(yaml, opts).expect_err("unknown anchor should error");
    let rendered = err.to_string();

    assert!(
        rendered.contains("*very_bad"),
        "expected snippet to include the offending token, got:\n{rendered}"
    );
    assert!(
        rendered.contains('…'),
        "expected cropped snippet to include ellipsis markers, got:\n{rendered}"
    );

    // The snippet window should include the surrounding context lines and apply the same
    // horizontal crop window to all of them (so they remain vertically aligned).
    // Assert that we can see cropped fragments from:
    // - the line before the error (before_2)
    // - the first two lines after the error (after_1, after_2)
    assert!(
        rendered.contains("6789ABC"),
        "expected snippet to include a cropped fragment from the line before the error, got:\n{rendered}"
    );
    assert!(
        rendered.contains("6789abc"),
        "expected snippet to include a cropped fragment from the first line after the error, got:\n{rendered}"
    );
    assert!(
        rendered.contains("_++_") || rendered.contains("*&^") || rendered.contains("%$#"),
        "expected snippet to include a cropped fragment from the second line after the error, got:\n{rendered}"
    );
    assert!(
        !rendered.contains("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "expected cropped snippet to omit the full long 'before' string, got:\n{rendered}"
    );
    assert!(
        !rendered.contains("0123456789abcdefghijklmnopqrstuvwxyz"),
        "expected cropped snippet to omit the full long 'after' string, got:\n{rendered}"
    );
}

/// Test that using `Spanned<T>` inside an untagged enum succeeds but loses location info.
/// Previously this would fail; now the fallback plain-value path in `Spanned<T>::deserialize`
/// allows it to succeed with `Location::UNKNOWN` for both `referenced` and `defined`.
#[test]
fn spanned_inside_untagged_enum_succeeds_with_unknown_location() {
    use serde_saphyr::Spanned;

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum PayloadWithSpanned {
        StringVariant { message: Spanned<String> },
        IntVariant { count: Spanned<u32> },
    }

    let yaml = "message: hello";

    // Location info is unavailable (Location::UNKNOWN)
    // because serde's untagged enum buffers values through ContentDeserializer which
    // discards the YAML deserializer context.
    let result = from_str::<PayloadWithSpanned>(yaml)
        .expect("Spanned inside untagged enum should now succeed");
    match result {
        PayloadWithSpanned::StringVariant { message } => {
            assert_eq!(message.value, "hello");
            assert_eq!(message.referenced.line(), 0, "location unavailable in untagged enum");
        }
        other => panic!("expected StringVariant, got {other:?}"),
    }
}

#[test]
fn alias_error_renders_two_snippet_windows_when_reference_and_defined_locations_differ() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Config {
        // Anchor value is valid for this field.
        count: u64,
        // Alias usage site triggers a type mismatch.
        flag: bool,
    }

    let yaml = "count: &val 42\nflag: *val\n";
    let opts = serde_saphyr::options! { with_snippet: true };

    let err = from_str_with_options::<Config>(yaml, opts)
        .expect_err("type mismatch at alias usage site should error");
    let rendered = err.to_string();

    // Dual-snippet rendering:
    // - primary window: alias usage site
    // - secondary window: anchor definition site
    assert!(
        rendered.contains("the value is used here"),
        "expected primary snippet label for alias usage site, got:\n{rendered}"
    );
    assert!(
        rendered.contains("defined here"),
        "expected secondary snippet label for anchor definition site, got:\n{rendered}"
    );

    // The output should include BOTH relevant source lines.
    assert!(
        rendered.contains("count: &val 42"),
        "expected rendered output to include anchor definition line, got:\n{rendered}"
    );
    assert!(
        rendered.contains("flag: *val"),
        "expected rendered output to include alias usage line, got:\n{rendered}"
    );

    // Heuristic: each snippet window has a caret marker.
    // (We don't assert exact formatting to keep the test robust across renderer tweaks.)
    let caret_count = rendered.matches('^').count();
    assert!(
        caret_count >= 2,
        "expected at least two caret markers (two snippet windows), got {caret_count}:\n{rendered}"
    );
}

#[cfg(feature = "garde")]
#[test]
fn garde_validation_error_renders_two_snippet_windows_when_value_is_anchored_and_aliased() {
    use garde::Validate;

    #[derive(Debug, Deserialize, Validate)]
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    struct ValidatedConfig {
        #[garde(skip)]
        name: String,
        #[garde(length(min = 2))]
        nickname: String,
    }

    // Anchor defines an invalid value ("x") and alias uses it for a validated field.
    let yaml = "name: &short \"x\"\nnickname: *short\n";
    let opts = serde_saphyr::options! { with_snippet: true };

    let err = serde_saphyr::from_str_with_options_valid::<ValidatedConfig>(yaml, opts)
        .expect_err("validation error expected");
    let rendered = err.to_string();

    assert!(
        rendered.contains("the value is used here"),
        "expected primary snippet label for alias usage site, got:\n{rendered}"
    );
    assert!(
        rendered.contains("defined here"),
        "expected secondary snippet label for anchor definition site, got:\n{rendered}"
    );

    assert!(
        rendered.contains("nickname: *short"),
        "expected rendered output to include alias usage line, got:\n{rendered}"
    );
    assert!(
        rendered.contains("name: &short"),
        "expected rendered output to include anchor definition line, got:\n{rendered}"
    );

    let caret_count = rendered.matches('^').count();
    assert!(
        caret_count >= 2,
        "expected at least two caret markers (two snippet windows), got {caret_count}:\n{rendered}"
    );
}
