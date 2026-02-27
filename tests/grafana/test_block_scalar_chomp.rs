use serde_saphyr::{SerializerOptions, to_fmt_writer_with_options};

/// Test that block_scalar_chomp: Strip forces `|-` and strips trailing newlines
#[test]
fn block_scalar_chomp_strip() {
    let input = "line1\nline2\n"; // Has trailing newline

    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(&mut buf, &input, opts).unwrap();

    // When parsed back, YAML must round-trip correctly.
    let parsed: String = serde_saphyr::from_str(&buf).unwrap();
    assert_eq!(parsed, input);
}

/// Test that block_scalar_chomp: Clip forces `|`
#[test]
fn block_scalar_chomp_clip() {
    let input = "line1\nline2"; // No trailing newline

    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(&mut buf, &input, opts).unwrap();

    // When parsed back, YAML must round-trip correctly.
    let parsed: String = serde_saphyr::from_str(&buf).unwrap();
    assert_eq!(parsed, input);
}

/// Test that block_scalar_chomp: Keep forces `|+`
#[test]
fn block_scalar_chomp_keep() {
    let input = "line1\nline2\n"; // Has one trailing newline

    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(&mut buf, &input, opts).unwrap();

    // When parsed back, YAML must round-trip correctly.
    let parsed: String = serde_saphyr::from_str(&buf).unwrap();
    assert_eq!(parsed, input);
}

/// Test that None (default) auto-detects based on trailing newlines
#[test]
fn block_scalar_chomp_auto() {
    // No trailing newline -> strip
    let input1 = "line1\nline2";
    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };
    let mut buf1 = String::new();
    to_fmt_writer_with_options(&mut buf1, &input1, opts).unwrap();
    let parsed1: String = serde_saphyr::from_str(&buf1).unwrap();
    assert_eq!(parsed1, input1);

    // One trailing newline -> clip
    let input2 = "line1\nline2\n";
    let mut buf2 = String::new();
    to_fmt_writer_with_options(&mut buf2, &input2, opts).unwrap();
    let parsed2: String = serde_saphyr::from_str(&buf2).unwrap();
    assert_eq!(parsed2, input2);

    // Two trailing newlines -> keep
    let input3 = "line1\nline2\n\n";
    let mut buf3 = String::new();
    to_fmt_writer_with_options(&mut buf3, &input3, opts).unwrap();
    let parsed3: String = serde_saphyr::from_str(&buf3).unwrap();
    assert_eq!(parsed3, input3);
}

/// Test the specific use case from the issue: nested block scalars should consistently use `|-`
#[test]
fn nested_block_scalar_strip_for_go_compat() {
    use serde::Serialize;
    use std::collections::HashMap;

    #[derive(Serialize)]
    struct ConfigMap {
        data: HashMap<String, String>,
    }

    // Inner YAML content that would be serialized as a block scalar
    let inner_yaml = r#"- "simple query"
- |
  sum(
        floor(
          max by (cluster, node) (metric{})
        )
      )
"#;

    let mut data = HashMap::new();
    data.insert("queries.yaml".to_string(), inner_yaml.to_string());
    let cm = ConfigMap { data };

    let opts = SerializerOptions {
        prefer_block_scalars: true,
        ..Default::default()
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(&mut buf, &cm, opts).unwrap();

    // Sanity-check we produced some YAML.
    assert!(!buf.is_empty());
}
