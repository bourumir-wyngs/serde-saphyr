use serde_saphyr::{self, SerializerOptions};

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
            len <= SerializerOptions::default().folded_wrap_chars,
            "line exceeds wrap width ({}): {:?}",
            SerializerOptions::default().folded_wrap_chars,
            line
        );
    }

    // Round-trip must preserve the original string
    let back: String = serde_saphyr::from_str(&out).unwrap();
    assert_eq!(back, long);
}
