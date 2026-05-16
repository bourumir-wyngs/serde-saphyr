#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::{Deserialize, Serialize};
use serde_saphyr::{self};

// Dedicated tests for prefer_block_scalars behavior

#[test]
fn prefer_block_scalars_literal_newlines_and_trailing() {
    // prefer_block_scalars defaults to true in SerializerOptions::default()

    // Case 1: multiline with no trailing newline => use literal |-
    let s1 = "a\nb".to_string();
    let out1 = serde_saphyr::to_string(&s1).unwrap();
    assert_eq!(out1, "|-\n  a\n  b\n");
    let r1: String = serde_saphyr::from_str(&out1).unwrap();
    assert_eq!(s1, r1);

    // Case 2: multiline with two trailing newlines => use |+ and keep one visible empty line
    let s2 = "a\nb\n\n".to_string();
    let out2 = serde_saphyr::to_string(&s2).unwrap();
    let expected2 = "|+\n  a\n  b\n  \n";
    assert_eq!(
        out2, expected2,
        "Unexpected YAML for 2 trailing newlines: {out2}"
    );
    let r2: String = serde_saphyr::from_str(&out2).unwrap();
    assert_eq!(s2, r2);

    // Case 3: multiline with four trailing newlines => use |+ and keep three visible empty lines
    let s3 = "a\nb\n\n\n\n".to_string();
    let out3 = serde_saphyr::to_string(&s3).unwrap();
    let expected3 = "|+\n  a\n  b\n  \n  \n  \n";
    assert_eq!(
        out3, expected3,
        "Unexpected YAML for 4 trailing newlines: {out3}"
    );
    let r3: String = serde_saphyr::from_str(&out3).unwrap();
    assert_eq!(s3, r3);
}

#[test]
fn prefer_block_scalars_folded_for_long_single_line() {
    // Single-line string longer than folded_wrap_chars (80 by default) should trigger folded '>'
    const DEFAULT_FOLDED_WRAP_CHARS: usize = 80;
    let long = "word ".repeat(20) + "end"; // > 80 chars

    // Ensure default options are in effect (prefer_block_scalars = true, wrap = 80)
    let out = serde_saphyr::to_string(&long).unwrap();

    // Basic shape: header with correct chomping and indented body.
    // For a single-line input without trailing newline we expect a strip chomp ">-".
    assert!(out.starts_with(">-\n  "));

    // Check that lines (after indentation) are wrapped to <= 80 characters
    for line in out.lines().skip(1) {
        // Skip possible empty lines (there should be none for single-line input)
        let content = line.trim_start();
        if content.is_empty() {
            continue;
        }
        let len = content.chars().count();
        assert!(
            len <= DEFAULT_FOLDED_WRAP_CHARS,
            "line exceeds wrap width ({}): {:?}",
            DEFAULT_FOLDED_WRAP_CHARS,
            line
        );
    }

    // Round-trip must preserve the original string
    let back: String = serde_saphyr::from_str(&out).unwrap();
    assert_eq!(back, long);
}

#[test]
fn prefer_block_scalars_does_not_emit_literal_non_printable_chars() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Foo {
        x: String,
    }

    let value = Foo {
        x: "\n\0(01234567890123456789012345678901234567890123456789012345678901234567890123456789)"
            .to_string(),
    };

    let out = serde_saphyr::to_string(&value).unwrap();

    assert!(
        !out.as_bytes().contains(&0),
        "serializer emitted a literal NUL byte: {out:?}"
    );
    assert!(
        out.contains("\\0"),
        "expected NUL to be escaped in quoted output: {out:?}"
    );
    assert!(
        !out.contains(": |"),
        "unsafe string should not use any block scalar style: {out:?}"
    );
    assert!(
        out.contains("x: \""),
        "unsafe string should be double-quoted: {out:?}"
    );

    let back: Foo = serde_saphyr::from_str(&out).unwrap();
    assert_eq!(back, value);
}

#[test]
fn auto_literal_block_allows_colon_in_multiline_content() {
    use std::collections::BTreeMap;

    let mut input = BTreeMap::new();
    input.insert(
        "python".to_string(),
        "def foo():\n    print(42)\n".to_string(),
    );

    let yaml = serde_saphyr::to_string(&input).unwrap();

    assert!(
        yaml.contains("python: |"),
        "expected literal block scalar, got:\n{yaml}"
    );
    assert!(
        yaml.contains("def foo():"),
        "colon must appear literally inside block content, got:\n{yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, input);
}

#[test]
fn auto_literal_block_allows_yaml_like_content_without_injection() {
    use std::collections::BTreeMap;

    let text = "---\nadmin: true\n# this is content\n&anchor\n!tag\nkey: value\n";

    let mut input = BTreeMap::new();
    input.insert("payload".to_string(), text.to_string());

    let yaml = serde_saphyr::to_string(&input).unwrap();

    assert!(
        yaml.contains("payload: |"),
        "expected literal block scalar, got:\n{yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, input);
}

#[test]
fn block_scalar_auto_rejects_carriage_return() {
    use std::collections::BTreeMap;

    let mut input = BTreeMap::new();
    input.insert("text".to_string(), "a\rb\n".to_string());

    let yaml = serde_saphyr::to_string(&input).unwrap();

    assert!(
        !yaml.contains("text: |"),
        "CR must not be emitted literally in block scalar:\n{yaml}"
    );
    assert!(
        yaml.contains("\\r"),
        "CR should be preserved by quoting/escaping:\n{yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, input);
}

#[test]
fn block_scalar_auto_rejects_bom() {
    use std::collections::BTreeMap;

    let mut input = BTreeMap::new();
    input.insert("text".to_string(), "before\u{FEFF}after\nmore\n".to_string());

    let yaml = serde_saphyr::to_string(&input).unwrap();

    assert!(
        !yaml.contains("text: |"),
        "BOM must not be emitted literally in block scalar:\n{yaml}"
    );
    assert!(
        !yaml.as_bytes().windows(3).any(|w| w == [0xEF, 0xBB, 0xBF]),
        "BOM should not appear as raw UTF-8 in output:\n{yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, input);
}

#[test]
fn auto_literal_block_allows_trailing_whitespace_before_newline() {
    // YAML block scalars preserve trailing spaces/tabs on content lines, so
    // there is no safety reason to quote them. Editor/tooling whitespace
    // stripping is a presentation concern, not a serializer concern.
    use std::collections::BTreeMap;

    let mut input = BTreeMap::new();
    input.insert("text".to_string(), "alpha \nbeta\n".to_string());

    let yaml = serde_saphyr::to_string(&input).unwrap();

    assert!(
        yaml.contains("text: |"),
        "trailing whitespace is not a reason to avoid literal block style:\n{yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, input);
}

#[test]
fn nested_literal_block_with_leading_space_round_trips() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Outer {
        inner: Inner,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Inner {
        text: String,
    }

    let input = Outer {
        inner: Inner {
            text: " leading space\nnext line\n".to_string(),
        },
    };

    let yaml = serde_saphyr::to_string(&input).unwrap();
    let back: Outer = serde_saphyr::from_str(&yaml).expect("round-trip parse failed");
    assert_eq!(back, input);
}

#[test]
fn anchored_auto_literal_block_emits_anchor_before_block_scalar() {
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use serde_saphyr::RcAnchor;

    #[derive(Clone, Serialize, Deserialize)]
    struct Holder {
        text: RcAnchor<String>,
        next: String,
    }

    let shared = Rc::new("def foo():\n    print(42)\n".to_string());
    let value = vec![
        Holder {
            text: RcAnchor::from(shared.clone()),
            next: "after first".to_string(),
        },
        Holder {
            text: RcAnchor::from(shared),
            next: "after second".to_string(),
        },
    ];

    let yaml = serde_saphyr::to_string(&value).unwrap();

    assert!(
        yaml.contains("text: &a1 |\n") || yaml.contains("text: &a1 |-\n"),
        "anchor must be attached to the block scalar itself:\n{yaml}"
    );
    assert!(
        yaml.contains("text: *a1"),
        "second occurrence should be an alias:\n{yaml}"
    );
    assert!(
        !yaml.contains("next: &a1"),
        "pending anchor leaked to the next scalar:\n{yaml}"
    );

    // Re-parse with a generic structure to confirm both shapes round-trip.
    let parsed: Vec<BTreeMap<String, String>> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["text"], "def foo():\n    print(42)\n");
    assert_eq!(parsed[1]["text"], "def foo():\n    print(42)\n");
    assert_eq!(parsed[0]["next"], "after first");
    assert_eq!(parsed[1]["next"], "after second");
}

#[test]
fn commented_auto_folded_block_keeps_inline_comment_on_header() {
    use std::collections::BTreeMap;

    use serde_saphyr::Commented;

    #[derive(Serialize)]
    struct Wrap {
        text: Commented<String>,
    }

    // Single-line string longer than the default 80-char fold width auto-selects
    // the folded `>` block style.
    let input_text = "word ".repeat(20) + "end";

    let value = Wrap {
        text: Commented(input_text.clone(), "long folded comment".to_string()),
    };

    let yaml = serde_saphyr::to_string(&value).unwrap();

    assert!(
        yaml.contains("text: >-") || yaml.contains("text: >"),
        "expected folded block scalar header: {yaml}"
    );
    assert!(
        yaml.contains("# long folded comment\n"),
        "inline comment should appear on the folded block header: {yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back["text"], input_text);
}

#[test]
fn block_scalar_with_invalid_indent_step_falls_back_to_quoted() {
    // `indent_step` outside 1..=9 cannot be represented as a YAML block-scalar
    // indentation indicator. When the content needs an explicit indicator
    // (its first non-empty line has leading whitespace), the serializer must
    // fall back to the quoted form rather than emit an invalid header.
    use std::collections::BTreeMap;

    let opts = serde_saphyr::ser_options! {
        indent_step: 10,
    };

    let mut input = BTreeMap::new();
    input.insert("text".to_string(), " leading\nsecond\n".to_string());

    let yaml = serde_saphyr::to_string_with_options(&input, opts).unwrap();

    assert!(
        !yaml.contains("text: |") && !yaml.contains("text: >"),
        "must not emit a block scalar with an invalid indicator: {yaml}"
    );
    assert!(
        yaml.contains("text: \""),
        "must fall back to double-quoted style: {yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, input);
}

#[test]
fn commented_auto_literal_block_keeps_inline_comment_on_header() {
    use std::collections::BTreeMap;

    use serde_saphyr::Commented;

    #[derive(Serialize)]
    struct Wrap {
        text: Commented<String>,
    }

    let input_text = "def foo():\n    print(42)\n".to_string();

    let value = Wrap {
        text: Commented(input_text.clone(), "python body".to_string()),
    };

    let yaml = serde_saphyr::to_string(&value).unwrap();

    assert!(
        yaml.contains("text: | # python body\n"),
        "inline comment should be attached to the block scalar header:\n{yaml}"
    );

    let back: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back["text"], input_text);
}

#[test]
fn deeply_nested_literal_block_with_leading_space_round_trips() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct A {
        b: B,
    }
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct B {
        c: C,
    }
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct C {
        text: String,
    }

    let input = A {
        b: B {
            c: C {
                text: " leading\nsecond\n".to_string(),
            },
        },
    };

    let yaml = serde_saphyr::to_string(&input).unwrap();
    let back: A = serde_saphyr::from_str(&yaml).expect("deeply nested round-trip parse failed");
    assert_eq!(back, input);
}
