#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::error::Error as StdError;

mod ser_error_tests {
    use super::*;
    use serde_saphyr::ser_error::Error;

    #[test]
    fn display_message() {
        let e = Error::Message {
            msg: "hello".into(),
        };
        assert_eq!(e.to_string(), "hello");
    }

    #[test]
    fn display_format() {
        let e = Error::Format {
            error: std::fmt::Error,
        };
        assert!(e.to_string().contains("formatting error"));
    }

    #[test]
    fn display_io() {
        let e = Error::IO {
            error: std::io::Error::other("disk full"),
        };
        let s = e.to_string();
        assert!(s.contains("I/O error"));
        assert!(s.contains("disk full"));
    }

    #[test]
    fn display_unexpected() {
        let e = Error::Unexpected {
            msg: "bad state".into(),
        };
        assert!(e.to_string().contains("unexpected internal error"));
    }

    #[test]
    fn display_invalid_options() {
        let e = Error::InvalidOptions("zero indent".into());
        assert!(e.to_string().contains("invalid serialization options"));
    }

    #[test]
    fn display_single_quoted_requires_escaping() {
        let e = Error::SingleQuotedRequiresEscaping { ch: '\n' };
        let s = e.to_string();
        assert!(s.contains("'\\n'"));
        assert!(!s.contains("\n'")); // not literally
    }

    #[test]
    fn source_delegates() {
        let fmt_err = Error::Format {
            error: std::fmt::Error,
        };
        assert!(fmt_err.source().is_some());

        let io_err = Error::IO {
            error: std::io::Error::other("x"),
        };
        assert!(io_err.source().is_some());

        let msg_err = Error::Message { msg: "m".into() };
        assert!(msg_err.source().is_none());

        let unexp = Error::Unexpected { msg: "u".into() };
        assert!(unexp.source().is_none());

        let inv = Error::InvalidOptions("i".into());
        assert!(inv.source().is_none());

        let single_quoted = Error::SingleQuotedRequiresEscaping { ch: '\n' };
        assert!(single_quoted.source().is_none());
    }

    #[test]
    fn from_fmt_error() {
        let e: Error = std::fmt::Error.into();
        assert!(matches!(e, Error::Format { .. }));
    }

    #[test]
    fn from_io_error() {
        let e: Error = std::io::Error::other("boom").into();
        assert!(matches!(e, Error::IO { .. }));
    }

    #[test]
    fn from_string() {
        let e: Error = String::from("oops").into();
        assert!(matches!(e, Error::Message { msg } if msg == "oops"));
    }

    #[test]
    fn from_ref_string() {
        let s = String::from("ref");
        let e: Error = (&s).into();
        assert!(matches!(e, Error::Message { msg } if msg == "ref"));
    }

    #[test]
    fn from_str_ref() {
        let e: Error = "literal".into();
        assert!(matches!(e, Error::Message { msg } if msg == "literal"));
    }

    #[test]
    fn serde_ser_error_custom() {
        use serde::ser::Error as _;
        let e = Error::custom("custom msg");
        assert_eq!(e.to_string(), "custom msg");
    }
}
