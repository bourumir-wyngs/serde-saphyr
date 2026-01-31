//! Tests for automatic line wrapping of long strings (matches Go's yaml.v3 SetWidth behavior)

use serde::{Deserialize, Serialize};
use serde_saphyr::{SerializerOptions, to_fmt_writer_with_options, to_string};

#[test]
fn line_width_none_does_not_wrap() {
    #[derive(Serialize)]
    struct Doc {
        description: String,
    }

    let long_text =
        "This is a very long description that would normally be wrapped at 80 characters.";
    let doc = Doc {
        description: long_text.to_string(),
    };

    // Default: line_width is None
    let out = to_string(&doc).unwrap();

    // Should be a single line (no wrapping)
    let content_lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        content_lines.len(),
        1,
        "Should not wrap by default, got:\n{}",
        out
    );
}

#[test]
fn line_width_short_strings_not_wrapped() {
    #[derive(Serialize)]
    struct Doc {
        name: String,
    }

    let doc = Doc {
        name: "short".to_string(),
    };

    let opts = SerializerOptions {
        
        ..Default::default()
    };

    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, &doc, opts).unwrap();

    // Short string should not be wrapped
    assert!(
        !out.contains(">\n"),
        "Short strings should not be wrapped, got:\n{}",
        out
    );
    assert!(out.contains("name: short"), "Expected plain scalar");
}

#[test]
fn line_width_with_nested_structure() {
    #[derive(Serialize, Deserialize, Debug)]
    struct Outer {
        inner: Inner,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Inner {
        description: String,
    }

    // Use a longer string that will definitely wrap
    let long_text = "This is a very long description that should be wrapped at approximately 80 characters to improve the overall readability and maintainability of the YAML output file.";
    let doc = Outer {
        inner: Inner {
            description: long_text.to_string(),
        },
    };

    let opts = SerializerOptions {
        
        ..Default::default()
    };

    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, &doc, opts).unwrap();

    println!("Output:\n{}", out);

    // Should wrap the string
    assert!(
        out.lines().count() > 2,
        "Expected wrapped string in nested structure, got:\n{}",
        out
    );

    // Should parse back correctly
    let parsed: Outer = serde_saphyr::from_str(&out).unwrap();
    assert!(parsed.inner.description.contains("long description"));
}

#[test]
fn line_width_custom_value() {
    #[derive(Serialize, Deserialize, Debug)]
    struct Doc {
        text: String,
    }

    // A string with multiple words that will wrap at a lower width
    let text = "This is a test string that has multiple words and will wrap when the line width is set to forty characters.";
    let doc = Doc {
        text: text.to_string(),
    };

    // With line_width=40, this should wrap
    let opts = SerializerOptions {
        
        ..Default::default()
    };

    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, &doc, opts).unwrap();

    println!("Output:\n{}", out);

    // Should wrap (plain scalar style)
    assert!(
        out.lines().count() > 1,
        "Expected wrapped string with line_width=40, got:\n{}",
        out
    );

    // Should parse back correctly
    let parsed: Doc = serde_saphyr::from_str(&out).unwrap();
    assert!(parsed.text.contains("test string"));
}

#[test]
fn line_width_preserves_multiline_strings() {
    #[derive(Serialize)]
    struct Doc {
        text: String,
    }

    // Multiline strings should still work with prefer_block_scalars
    let doc = Doc {
        text: "line1\nline2\nline3".to_string(),
    };

    let opts = SerializerOptions {
        
        prefer_block_scalars: true,
        ..Default::default()
    };

    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, &doc, opts).unwrap();

    // Should use literal block scalar for multiline
    assert!(
        out.contains("|-") || out.contains("|"),
        "Expected literal block scalar for multiline, got:\n{}",
        out
    );
}

#[test]
fn line_width_flow_style_not_wrapped() {
    use serde_saphyr::FlowSeq;

    #[derive(Serialize)]
    struct Doc {
        items: FlowSeq<Vec<String>>,
    }

    let long_text =
        "This is a very long string that would normally be wrapped but not in flow style.";
    let doc = Doc {
        items: FlowSeq(vec![long_text.to_string()]),
    };

    let opts = SerializerOptions {
        
        ..Default::default()
    };

    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, &doc, opts).unwrap();

    // Flow style should not use block scalars
    assert!(
        out.contains("["),
        "Expected flow style sequence, got:\n{}",
        out
    );
    assert!(
        !out.contains(">\n"),
        "Flow style should not use folded block scalars"
    );
}

#[test]
fn line_width_roundtrip() {
    #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Doc {
        description: String,
    }

    let original_text = "This is a very long description that will be wrapped when serialized but should deserialize back to the original content when parsed.";
    let doc = Doc {
        description: original_text.to_string(),
    };

    let opts = SerializerOptions {
        
        ..Default::default()
    };

    let mut yaml = String::new();
    to_fmt_writer_with_options(&mut yaml, &doc, opts).unwrap();

    // Parse it back
    let parsed: Doc = serde_saphyr::from_str(&yaml).unwrap();

    // Plain multi-line scalars fold newlines to spaces, so we compare word content
    let original_words: Vec<&str> = original_text.split_whitespace().collect();
    let parsed_words: Vec<&str> = parsed.description.split_whitespace().collect();
    assert_eq!(
        original_words, parsed_words,
        "Content should be preserved after roundtrip"
    );
}
