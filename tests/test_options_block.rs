use serde_saphyr::{
    to_fmt_writer_with_options, FoldStr, FoldString, LitStr, LitString, SerializerOptions,
};

#[test]
fn lit_wrappers_respect_min_fold_chars_option() {
    // Default: threshold 32, so a short single-line becomes plain scalar.
    let mut s = String::new();
    to_fmt_writer_with_options(&mut s, &LitStr("short"), SerializerOptions::default()).unwrap();
    assert_eq!(s, "short\n");

    // With min_fold_chars = 0, even a short single-line should use block style `|`.
    let opts = SerializerOptions {
        min_fold_chars: 0,
        ..SerializerOptions::default()
    };
    s.clear();
    to_fmt_writer_with_options(&mut s, &LitStr("short"), opts).unwrap();
    assert_eq!(s, "|\n  short\n");

    // Newlines always force block style regardless of threshold.
    s.clear();
    to_fmt_writer_with_options(
        &mut s,
        &LitStr("a\nb"),
        SerializerOptions {
            min_fold_chars: usize::MAX,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s, "|\n  a\n  b\n");
}

#[test]
fn lit_owned_variant_also_respects_option() {
    let mut s = String::new();
    let opts = SerializerOptions {
        min_fold_chars: 0,
        ..SerializerOptions::default()
    };
    to_fmt_writer_with_options(&mut s, &LitString("ok".to_string()), opts).unwrap();
    assert_eq!(s, "|\n  ok\n");
}

#[test]
fn fold_wrapping_uses_configured_column() {
    // Configure very small wrap to make behavior easy to assert
    let opts = SerializerOptions {
        folded_wrap_chars: 10,
        min_fold_chars: 0,
        ..SerializerOptions::default()
    };

    // A single long line without newlines should still go to block style because min_fold_chars=0
    // and then wrap at <=10 columns with word boundaries when possible.
    let text = "Mazurka seeds were germinated"; // has spaces to wrap on
    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, &FoldStr(text), opts).unwrap();
    // Expect a folded block header and wrapped lines indented by two spaces
    assert!(out.starts_with(">\n  "));
    for line in out.lines().skip(1) {
        // skip the '>' header line
        if line.trim().is_empty() {
            continue;
        }
        // no line (after indentation) should exceed 10 chars
        let content_len = line.trim_start().chars().count();
        assert!(content_len <= 10, "line too long: {:?}", line);
    }
}

#[test]
fn fold_owned_variant_respects_wrap() {
    let opts = SerializerOptions {
        folded_wrap_chars: 12,
        min_fold_chars: 0,
        ..SerializerOptions::default()
    };
    let mut out = String::new();
    to_fmt_writer_with_options(
        &mut out,
        &FoldString("alpha beta gamma delta".to_string()),
        opts,
    )
    .unwrap();
    // Basic sanity: header + at least two lines due to wrap <=12
    assert!(out.starts_with(">\n  "));
    assert!(out.lines().count() >= 3);
}
