#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Deserialize;

mod with_deserializer_tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Simple {
        x: i32,
    }

    #[test]
    fn from_str_basic() {
        let result: Simple =
            serde_saphyr::with_deserializer_from_str("x: 42", |de| Simple::deserialize(de))
                .unwrap();
        assert_eq!(result, Simple { x: 42 });
    }

    #[test]
    fn from_str_bom() {
        // UTF-8 BOM should be stripped
        let input = "\u{FEFF}x: 99";
        let result: Simple =
            serde_saphyr::with_deserializer_from_str(input, |de| Simple::deserialize(de)).unwrap();
        assert_eq!(result, Simple { x: 99 });
    }

    #[test]
    fn from_slice_basic() {
        let bytes = b"x: 7";
        let result: Simple =
            serde_saphyr::with_deserializer_from_slice(bytes, |de| Simple::deserialize(de))
                .unwrap();
        assert_eq!(result, Simple { x: 7 });
    }

    #[test]
    fn from_slice_invalid_utf8() {
        let bytes: &[u8] = &[0xFF, 0xFE, 0x00];
        let err = serde_saphyr::with_deserializer_from_slice(bytes, |de| Simple::deserialize(de))
            .unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("UTF-8") || s.contains("utf8") || s.contains("utf-8"),
            "expected UTF-8 error, got: {s}"
        );
    }

    #[test]
    fn from_reader_basic() {
        let data = b"x: 3";
        let cursor = std::io::Cursor::new(data);
        let result: Simple =
            serde_saphyr::with_deserializer_from_reader(cursor, |de| Simple::deserialize(de))
                .unwrap();
        assert_eq!(result, Simple { x: 3 });
    }

    #[test]
    fn from_str_multiple_documents_error() {
        let input = "x: 1\n---\nx: 2";
        let err = serde_saphyr::with_deserializer_from_str(input, |de| Simple::deserialize(de))
            .unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("multiple") || s.contains("document"),
            "expected multiple documents error, got: {s}"
        );
    }

    #[test]
    fn from_str_empty_eof() {
        // Empty input into bool should give EOF error
        let err =
            serde_saphyr::with_deserializer_from_str("", |de| bool::deserialize(de)).unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("end") || s.contains("EOF") || s.contains("eof"),
            "expected EOF error, got: {s}"
        );
    }

    #[test]
    fn from_str_with_options() {
        let opts = serde_saphyr::options! {};
        let result: Simple =
            serde_saphyr::with_deserializer_from_str_with_options("x: 5", opts, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 5 });
    }

    #[test]
    fn from_slice_with_options() {
        let opts = serde_saphyr::options! {};
        let result: Simple =
            serde_saphyr::with_deserializer_from_slice_with_options(b"x: 8", opts, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 8 });
    }

    #[test]
    fn from_reader_with_options() {
        let opts = serde_saphyr::options! {};
        let cursor = std::io::Cursor::new(b"x: 11");
        let result: Simple =
            serde_saphyr::with_deserializer_from_reader_with_options(cursor, opts, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 11 });
    }

    #[test]
    fn from_reader_empty_eof_without_snippet() {
        let opts = serde_saphyr::options! {
            with_snippet: false,
        };
        let cursor = std::io::Cursor::new(Vec::<u8>::new());
        let err = serde_saphyr::with_deserializer_from_reader_with_options(cursor, opts, |de| {
            bool::deserialize(de)
        })
        .unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("end") || s.contains("EOF") || s.contains("eof"),
            "expected EOF error, got: {s}"
        );
    }

    #[test]
    fn from_reader_multiple_documents_error() {
        let cursor = std::io::Cursor::new(b"x: 1\n---\nx: 2\n");
        let err = serde_saphyr::with_deserializer_from_reader(cursor, |de| Simple::deserialize(de))
            .unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("multiple") || s.contains("document"),
            "expected multiple documents error, got: {s}"
        );
    }

    #[test]
    fn from_str_trailing_garbage_after_document_end_is_ignored() {
        let result: Simple = serde_saphyr::with_deserializer_from_str(
            "x: 12\n...\n@ ignored after document end\n",
            |de| Simple::deserialize(de),
        )
        .unwrap();
        assert_eq!(result, Simple { x: 12 });
    }

    #[test]
    fn from_str_trailing_garbage_without_document_end_errors() {
        let err =
            serde_saphyr::with_deserializer_from_str("x: 1\n@\n", |de| Simple::deserialize(de))
                .unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}
