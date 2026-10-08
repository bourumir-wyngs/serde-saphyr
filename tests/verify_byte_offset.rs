#![cfg(all(feature = "serialize", feature = "deserialize"))]
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

#[rstest::rstest]
#[case::without_bom("")]
#[case::with_bom("\u{FEFF}")]
fn test_byte_offset_after_bom(#[case] prefix: &str) {
    // A leading BOM is part of the caller's string: byte and character offsets must count it,
    // so the span slices the original input back to the value.
    let input = format!("{prefix}foo: bar\n");
    #[derive(serde::Deserialize)]
    struct Test {
        foo: Spanned<String>,
    }

    let t: Test = serde_saphyr::from_str(&input).unwrap();
    assert_eq!((t.foo.referenced.line(), t.foo.referenced.column()), (1, 6));
    let span = t.foo.referenced.span();
    let (off, len) = (span.byte_offset().unwrap(), span.byte_len().unwrap());
    assert_eq!((off, len), (5 + prefix.len() as u64, 3));
    assert_eq!(&input[off as usize..(off + len) as usize], "bar");
    assert_eq!(span.offset(), 5 + prefix.chars().count() as u64);

    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Small {
        n: u8,
    }
    let input = format!("{prefix}n: 300\n");
    let err = serde_saphyr::from_str::<Small>(&input).unwrap_err();
    let location = err.location().unwrap();
    assert_eq!((location.line(), location.column()), (1, 4));
    let span = location.span();
    let (off, len) = (span.byte_offset().unwrap(), span.byte_len().unwrap());
    assert_eq!((off, len), (3 + prefix.len() as u64, 3));
    assert_eq!(span.offset(), 3 + prefix.chars().count() as u64);
    assert_eq!(&input[off as usize..(off + len) as usize], "300");
}

#[rstest::rstest]
#[case::without_bom("")]
#[case::with_bom("\u{FEFF}")]
fn test_byte_offset_after_bom_multiple_documents(#[case] prefix: &str) {
    #[derive(serde::Deserialize)]
    struct Test {
        foo: Spanned<String>,
    }

    let input = format!("{prefix}foo: bar\n---\nfoo: baz\n");
    let docs: Vec<Test> = serde_saphyr::from_str_multiple(&input).unwrap();
    assert_eq!(docs.len(), 2);
    for (doc, (offset, line, value)) in docs.iter().zip([(5, 1, "bar"), (18, 3, "baz")]) {
        assert_eq!(doc.foo.value, value);
        let location = doc.foo.referenced;
        assert_eq!((location.line(), location.column()), (line, 6));
        let span = location.span();
        let (off, len) = (span.byte_offset().unwrap(), span.byte_len().unwrap());
        assert_eq!((off, len), (offset + prefix.len() as u64, 3));
        assert_eq!(span.offset(), offset + prefix.chars().count() as u64);
        assert_eq!(&input[off as usize..(off + len) as usize], value);
    }

    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Small {
        n: u8,
    }

    let input = format!("{prefix}n: 1\n---\nn: 300\n");
    let err = serde_saphyr::from_str_multiple::<Small, Vec<_>>(&input).unwrap_err();
    let location = err.location().unwrap();
    assert_eq!((location.line(), location.column()), (3, 4));
    let span = location.span();
    let (off, len) = (span.byte_offset().unwrap(), span.byte_len().unwrap());
    assert_eq!((off, len), (12 + prefix.len() as u64, 3));
    assert_eq!(span.offset(), 12 + prefix.chars().count() as u64);
    assert_eq!(&input[off as usize..(off + len) as usize], "300");
}

#[cfg(any(feature = "garde", feature = "validator"))]
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
struct BomValidated {
    #[cfg_attr(feature = "garde", garde(length(min = 2)))]
    #[cfg_attr(feature = "validator", validate(length(min = 2)))]
    key: String,
}

#[cfg(any(feature = "garde", feature = "validator"))]
#[rstest::rstest]
#[cfg_attr(feature = "garde", case::garde(
    serde_saphyr::from_str_valid::<BomValidated>,
    serde_saphyr::from_multiple_valid::<BomValidated>,
))]
#[cfg_attr(feature = "validator", case::validator(
    serde_saphyr::from_str_validate::<BomValidated>,
    serde_saphyr::from_multiple_validate::<BomValidated>,
))]
fn test_byte_offset_after_bom_validation(
    #[case] single: fn(&str) -> Result<BomValidated, serde_saphyr::Error>,
    #[case] multiple: fn(&str) -> Result<Vec<BomValidated>, serde_saphyr::Error>,
    #[values("", "\u{FEFF}")] prefix: &str,
) {
    let input = format!("{prefix}key: x\n");
    let err = single(&input).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        serde_saphyr::Error::ValidationError { .. }
    ));
    let location = err.location().unwrap();
    assert_eq!((location.line(), location.column()), (1, 6));
    let span = location.span();
    let (off, len) = (span.byte_offset().unwrap(), span.byte_len().unwrap());
    assert_eq!((off, len), (5 + prefix.len() as u64, 1));
    assert_eq!(span.offset(), 5 + prefix.chars().count() as u64);
    assert_eq!(&input[off as usize..(off + len) as usize], "x");

    let input = format!("{prefix}key: x\n---\nkey: y\n");
    let err = multiple(&input).unwrap_err();
    let serde_saphyr::Error::ValidationErrors { errors, .. } = err.without_snippet() else {
        panic!("expected validation errors for both documents, got: {err:?}");
    };
    assert_eq!(errors.len(), 2);
    for (err, (offset, line, value)) in errors.iter().zip([(5, 1, "x"), (16, 3, "y")]) {
        assert!(matches!(
            err.without_snippet(),
            serde_saphyr::Error::ValidationError { .. }
        ));
        let location = err.location().unwrap();
        assert_eq!((location.line(), location.column()), (line, 6));
        let span = location.span();
        let (off, len) = (span.byte_offset().unwrap(), span.byte_len().unwrap());
        assert_eq!((off, len), (offset + prefix.len() as u64, 1));
        assert_eq!(span.offset(), offset + prefix.chars().count() as u64);
        assert_eq!(&input[off as usize..(off + len) as usize], value);
    }
}

#[cfg(feature = "miette")]
#[rstest::rstest]
#[case::without_bom("")]
#[case::with_bom("\u{FEFF}")]
fn test_byte_offset_after_bom_miette_without_snippets(#[case] prefix: &str) {
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Small {
        n: u8,
    }

    let input = format!("{prefix}n: 300\n");
    let err = serde_saphyr::from_str_with_options::<Small>(
        &input,
        serde_saphyr::options! { with_snippet: false },
    )
    .unwrap_err();
    assert!(!matches!(err, serde_saphyr::Error::WithSnippet { .. }));
    let location = err.location().unwrap();
    assert_eq!((location.line(), location.column()), (1, 4));
    assert_eq!(location.span().byte_offset(), Some(3 + prefix.len() as u64));
    assert_eq!(location.span().byte_len(), Some(3));
    assert_eq!(location.span().offset(), 3 + prefix.chars().count() as u64);

    let report = serde_saphyr::miette::to_miette_report(&err, &input, "bom.yaml");
    let labels: Vec<_> = report.labels().expect("error label").collect();
    assert_eq!(labels.len(), 1);
    let label = &labels[0];
    assert_eq!((label.offset(), label.len()), (3 + prefix.len(), 3));
    assert_eq!(&input[label.offset()..label.offset() + label.len()], "300");
}
