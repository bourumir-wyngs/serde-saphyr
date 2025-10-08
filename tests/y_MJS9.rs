// MJS9: Block Folding — expect "foo \n\n\t bar\n\nbaz\n"
// Note: The suite uses visible space markers; we embed actual spaces and a TAB.

#[test]
#[ignore]
fn yaml_mjs9_block_folding() {
    let y = r#">
  foo 
 
   	 bar
 
  baz
"#;
    let s: String = serde_saphyr::from_str(y).expect("failed to parse MJS9");
    assert_eq!(s, "foo \n\n\t bar\n\nbaz\n");
}

// Block folding tests with chomping indicators.
// - ">"  : Folded, clip (default). Keep 1 trailing newline.
// - ">-" : Folded, strip. Remove ALL trailing newlines.
// - ">+" : Folded, keep. Preserve ALL trailing newlines, even blank ones.

#[test]
#[ignore]
fn yaml_mjs9_block_folding_clip() {
    // Default ">" with no indicator = "clip"
    // Behavior: fold newlines into spaces (except empty lines),
    // keep *one* trailing newline at the end.
    let y = r#">
  foo

   	 bar

  baz
"#;
    let s: String = serde_saphyr::from_str(y).expect("failed to parse MJS9 clip");
    // Expect: "foo␣" then blank line, TAB+bar, blank line, "baz", final newline
    assert_eq!(s, "foo \n\n\t bar\n\nbaz\n");
}

#[test]
#[ignore]
fn yaml_mjs9_block_folding_strip() {
    // ">-" means folded with "strip" chomping.
    // Behavior: same folding rules, but remove ALL trailing newlines.
    let y = r#">-
  foo

   	 bar

  baz
"#;
    let s: String = serde_saphyr::from_str(y).expect("failed to parse MJS9 strip");
    // Expect same as clip version, but *no* final newline
    assert_eq!(s, "foo \n\n\t bar\n\nbaz");
}

#[test]
#[ignore]
fn yaml_mjs9_block_folding_keep() {
    // ">+" means folded with "keep" chomping.
    // Behavior: preserve ALL trailing newlines (so if the block ends with
    // a blank line, you get an *extra* newline in the result).
    let y = r#">+
  foo

   	 bar

  baz

"#;
    let s: String = serde_saphyr::from_str(y).expect("failed to parse MJS9 keep");
    // Here: content ends with an extra blank line, so YAML keeps *two* newlines
    // at the end instead of one.
    assert_eq!(s, "foo \n\n\t bar\n\nbaz\n\n");
}
