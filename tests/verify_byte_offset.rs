use serde_saphyr::Spanned;

#[test]
fn test_byte_offset() {
    let input = "foo: bar";
    #[derive(serde::Deserialize)]
    struct Test {
        foo: Spanned<String>,
    }

    let t: Test = serde_saphyr::from_str(input).unwrap();
    let span = t.foo.referenced.span();

    // "bar" is at index 5.
    // 'f'(0), 'o'(1), 'o'(2), ':'(3), ' '(4), 'b'(5)

    assert_eq!(
        span.byte_offset(),
        Some(5u64),
        "Offset for 'bar' should be 5"
    );
    assert_eq!(span.byte_len(), Some(3u64), "Length for 'bar' should be 3");
}

#[test]
fn test_multibyte() {
    // "€" is 3 bytes: E2 82 AC
    let input = "key: €";
    #[derive(serde::Deserialize)]
    struct Test {
        key: Spanned<String>,
    }

    let t: Test = serde_saphyr::from_str(input).unwrap();
    let span = t.key.referenced.span();

    // "key: " is 5 chars, 5 bytes.
    // "€" starts at byte 5.

    assert_eq!(span.byte_offset(), Some(5u64), "Offset for '€' should be 5");
    assert_eq!(
        span.byte_len(),
        Some(3u64),
        "Length for '€' should be 3 bytes"
    );
}

#[test]
fn test_multibyte_key() {
    // "€: val"
    // € is 3 bytes.
    let input = "€: val";
    #[derive(serde::Deserialize)]
    struct Test {
        #[serde(rename = "€")]
        key: Spanned<String>,
    }

    let t: Test = serde_saphyr::from_str(input).unwrap();
    let span = t.key.referenced.span();

    // "€: " is 3 bytes + 2 bytes = 5 bytes.
    // "val" starts at byte 5.

    assert_eq!(
        span.byte_offset(),
        Some(5u64),
        "Offset for 'val' should be 5"
    );
    assert_eq!(span.byte_len(), Some(3u64), "Length for 'val' should be 3");
}
