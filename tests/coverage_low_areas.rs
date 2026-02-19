//! Tests targeting the lowest-coverage source files:
//! ser_error, zmij_format (via serialization), localizer, and with_deserializer APIs.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::error::Error as StdError;

// ─── ser_error ───────────────────────────────────────────────────────────────

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
    fn source_delegates() {
        let fmt_err = Error::Format {
            error: std::fmt::Error,
        };
        assert!(fmt_err.source().is_some());

        let io_err = Error::IO {
            error: std::io::Error::other("x"),
        };
        assert!(io_err.source().is_some());

        let msg_err = Error::Message {
            msg: "m".into(),
        };
        assert!(msg_err.source().is_none());

        let unexp = Error::Unexpected {
            msg: "u".into(),
        };
        assert!(unexp.source().is_none());

        let inv = Error::InvalidOptions("i".into());
        assert!(inv.source().is_none());
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

// ─── zmij_format (exercised through serialization round-trips) ───────────────

mod zmij_format_tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Wrapper {
        v: f64,
    }

    fn round_trip(val: f64) -> String {
        let w = Wrapper { v: val };
        serde_saphyr::to_string(&w).unwrap()
    }

    #[test]
    fn nan() {
        let s = round_trip(f64::NAN);
        assert!(s.contains(".nan"), "expected .nan, got: {s}");
    }

    #[test]
    fn positive_inf() {
        let s = round_trip(f64::INFINITY);
        assert!(s.contains(".inf"), "expected .inf, got: {s}");
    }

    #[test]
    fn negative_inf() {
        let s = round_trip(f64::NEG_INFINITY);
        assert!(s.contains("-.inf"), "expected -.inf, got: {s}");
    }

    #[test]
    fn zero() {
        let s = round_trip(0.0);
        // Must contain a decimal point
        assert!(s.contains('.'), "expected decimal point, got: {s}");
    }

    #[test]
    fn small_exponent() {
        // 4e-6 should become 4.0e-6 (decimal point inserted)
        let s = round_trip(4e-6);
        assert!(s.contains('.'), "expected decimal point, got: {s}");
    }

    #[test]
    fn large_exponent() {
        let s = round_trip(1e20);
        // Should have exponent sign
        assert!(
            s.contains("e+") || s.contains("e-") || s.contains("E+") || s.contains("E-") || s.contains('.'),
            "expected proper float format, got: {s}"
        );
    }

    #[test]
    fn regular_float() {
        let s = round_trip(3.14);
        assert!(s.contains("3.14"), "expected 3.14, got: {s}");
    }

    #[test]
    fn integer_like_float() {
        // 1.0 should keep decimal point
        let s = round_trip(1.0);
        assert!(s.contains('.'), "expected decimal point for 1.0, got: {s}");
    }

    #[test]
    fn f32_nan() {
        #[derive(Serialize)]
        struct W32 {
            v: f32,
        }
        let s = serde_saphyr::to_string(&W32 { v: f32::NAN }).unwrap();
        assert!(s.contains(".nan"));
    }

    #[test]
    fn f32_inf() {
        #[derive(Serialize)]
        struct W32 {
            v: f32,
        }
        let s = serde_saphyr::to_string(&W32 { v: f32::INFINITY }).unwrap();
        assert!(s.contains(".inf"));
    }

    #[test]
    fn f32_neg_inf() {
        #[derive(Serialize)]
        struct W32 {
            v: f32,
        }
        let s = serde_saphyr::to_string(&W32 { v: f32::NEG_INFINITY }).unwrap();
        assert!(s.contains("-.inf"));
    }

    /// Exercise the write_float_string path via to_writer (fmt::Write)
    #[test]
    fn write_path_nan() {
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        let mut buf = String::new();
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: f64::NAN }).unwrap();
        assert!(buf.contains(".nan"));
    }

    #[test]
    fn write_path_inf() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: f64::INFINITY }).unwrap();
        assert!(buf.contains(".inf"));
    }

    #[test]
    fn write_path_neg_inf() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: f64::NEG_INFINITY }).unwrap();
        assert!(buf.contains("-.inf"));
    }

    #[test]
    fn write_path_small_exponent() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 4e-6 }).unwrap();
        assert!(buf.contains('.'), "expected decimal point, got: {buf}");
    }

    #[test]
    fn write_path_integer_like() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1.0 }).unwrap();
        assert!(buf.contains('.'), "expected decimal point, got: {buf}");
    }

    #[test]
    fn write_path_large_exponent() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1e20 }).unwrap();
        assert!(buf.contains("e+"), "expected e+ exponent sign, got: {buf}");
    }

    #[test]
    fn write_path_scientific_decimal_pos() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1.23e20 }).unwrap();
        assert!(buf.contains("e+"), "expected e+ exponent sign with decimal mantissa, got: {buf}");
    }

    #[test]
    fn write_path_scientific_decimal_neg() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1.23e-10 }).unwrap();
        assert!(buf.contains("e-"), "expected e- exponent sign, got: {buf}");
    }

    #[test]
    fn write_path_f32_large_exp() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f32,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1e20f32 }).unwrap();
        assert!(buf.contains("e"), "expected scientific notation, got: {buf}");
    }

    #[test]
    fn float_map_keys() {
        use serde::Serializer;
        use std::hash::{Hash, Hasher};
        use std::collections::HashMap;

        struct DummyF64(pub f64);

        impl PartialEq for DummyF64 {
            fn eq(&self, other: &Self) -> bool {
                self.0.to_bits() == other.0.to_bits()
            }
        }

        impl Eq for DummyF64 {}

        impl Hash for DummyF64 {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.to_bits().hash(state)
            }
        }

        impl serde::Serialize for DummyF64 {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_f64(self.0)
            }
        }

        let mut map: HashMap<DummyF64, String> = HashMap::new();
        map.insert(DummyF64(1.0), "one".to_string());
        map.insert(DummyF64(1e6), "million".to_string());
        map.insert(DummyF64(4e-6), "small".to_string());
        let yaml = serde_saphyr::to_string(&map).unwrap();
        assert!(yaml.contains("one"));
        assert!(yaml.contains("million"));
        assert!(yaml.contains("small"));
        // Covers push_float_string via KeyScalarSink for float keys
    }

    #[test]
    fn round_trip_scientific_decimal_pos() {
        let s = round_trip(1.23e20);
        assert!(s.contains("e+"), "expected e+ exponent sign, got: {s}");
    }

    #[test]
    fn round_trip_scientific_decimal_neg() {
        let s = round_trip(1.23e-10);
        assert!(s.contains("e-"), "expected e- exponent sign, got: {s}");
    }
}

// ─── localizer ───────────────────────────────────────────────────────────────

mod localizer_tests {
    use super::*;
    use serde_saphyr::localizer::{
        DefaultEnglishLocalizer, ExternalMessage, ExternalMessageSource, Localizer,
        DEFAULT_ENGLISH_LOCALIZER,
    };
    use serde_saphyr::Location;

    #[test]
    fn attach_location_unknown() {
        let l = &DEFAULT_ENGLISH_LOCALIZER;
        let result = l.attach_location(Cow::Borrowed("base"), Location::UNKNOWN);
        assert_eq!(result, "base");
    }


    #[test]
    fn root_path_label() {
        assert_eq!(DEFAULT_ENGLISH_LOCALIZER.root_path_label(), "<root>");
    }


    #[test]
    fn validation_issue_line_no_location() {
        let s = DEFAULT_ENGLISH_LOCALIZER.validation_issue_line("root", "missing", None);
        assert!(s.contains("validation error at root: missing"));
        assert!(!s.contains("line"));
    }

    #[test]
    fn validation_issue_line_unknown_location() {
        let s =
            DEFAULT_ENGLISH_LOCALIZER.validation_issue_line("x", "y", Some(Location::UNKNOWN));
        assert!(!s.contains("at line"));
    }

    #[test]
    fn join_validation_issues() {
        let lines = vec!["a".into(), "b".into()];
        assert_eq!(DEFAULT_ENGLISH_LOCALIZER.join_validation_issues(&lines), "a\nb");
    }

    #[test]
    fn snippet_labels() {
        let l = &DEFAULT_ENGLISH_LOCALIZER;
        assert_eq!(l.defined(), "(defined)");
        assert_eq!(l.defined_here(), "(defined here)");
        assert_eq!(l.value_used_here(), "the value is used here");
        assert_eq!(l.defined_window(), "defined here");
    }

    #[test]
    fn validation_base_message() {
        let s = DEFAULT_ENGLISH_LOCALIZER.validation_base_message("too short", "name");
        assert!(s.contains("validation error"));
        assert!(s.contains("too short"));
        assert!(s.contains("`name`"));
    }

    #[test]
    fn invalid_here() {
        let s = DEFAULT_ENGLISH_LOCALIZER.invalid_here("must be positive");
        assert!(s.contains("invalid here"));
        assert!(s.contains("must be positive"));
    }

    #[test]
    fn snippet_location_prefix_unknown() {
        let s = DEFAULT_ENGLISH_LOCALIZER.snippet_location_prefix(Location::UNKNOWN);
        assert!(s.is_empty());
    }


    #[test]
    fn override_external_message_default_none() {
        let msg = ExternalMessage {
            source: ExternalMessageSource::SaphyrParser,
            original: "scan error",
            code: None,
            params: &[],
        };
        assert!(DEFAULT_ENGLISH_LOCALIZER.override_external_message(msg).is_none());
    }

    #[test]
    fn default_english_localizer_is_debug_clone_copy() {
        let l = DefaultEnglishLocalizer;
        let _ = format!("{:?}", l);
        let l2 = l;
        let _ = l2;
    }

    /// Custom localizer that overrides one method.
    struct SpanishLocalizer;
    impl Localizer for SpanishLocalizer {
        fn root_path_label(&self) -> Cow<'static, str> {
            Cow::Borrowed("<raíz>")
        }
    }

    #[test]
    fn custom_localizer_override() {
        let l = SpanishLocalizer;
        assert_eq!(l.root_path_label(), "<raíz>");
        // Other methods still return English defaults
        assert_eq!(l.defined(), "(defined)");
    }
}

// ─── with_deserializer ──────────────────────────────────────────────────────

mod with_deserializer_tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Simple {
        x: i32,
    }

    #[test]
    fn from_str_basic() {
        let result: Simple =
            serde_saphyr::with_deserializer_from_str("x: 42", |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 42 });
    }

    #[test]
    fn from_str_bom() {
        // UTF-8 BOM should be stripped
        let input = "\u{FEFF}x: 99";
        let result: Simple =
            serde_saphyr::with_deserializer_from_str(input, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 99 });
    }

    #[test]
    fn from_slice_basic() {
        let bytes = b"x: 7";
        let result: Simple =
            serde_saphyr::with_deserializer_from_slice(bytes, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 7 });
    }

    #[test]
    fn from_slice_invalid_utf8() {
        let bytes: &[u8] = &[0xFF, 0xFE, 0x00];
        let err = serde_saphyr::with_deserializer_from_slice(bytes, |de| {
            Simple::deserialize(de)
        })
        .unwrap_err();
        let s = err.to_string();
        assert!(s.contains("UTF-8") || s.contains("utf8") || s.contains("utf-8"),
            "expected UTF-8 error, got: {s}");
    }

    #[test]
    fn from_reader_basic() {
        let data = b"x: 3";
        let cursor = std::io::Cursor::new(data);
        let result: Simple =
            serde_saphyr::with_deserializer_from_reader(cursor, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 3 });
    }

    #[test]
    fn from_str_multiple_documents_error() {
        let input = "x: 1\n---\nx: 2";
        let err = serde_saphyr::with_deserializer_from_str(input, |de| {
            Simple::deserialize(de)
        })
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
        let err = serde_saphyr::with_deserializer_from_str("", |de| {
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
    fn from_str_with_options() {
        let opts = serde_saphyr::Options::default();
        let result: Simple =
            serde_saphyr::with_deserializer_from_str_with_options("x: 5", opts, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 5 });
    }

    #[test]
    fn from_slice_with_options() {
        let opts = serde_saphyr::Options::default();
        let result: Simple =
            serde_saphyr::with_deserializer_from_slice_with_options(b"x: 8", opts, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 8 });
    }

    #[test]
    fn from_reader_with_options() {
        let opts = serde_saphyr::Options::default();
        let cursor = std::io::Cursor::new(b"x: 11");
        let result: Simple =
            serde_saphyr::with_deserializer_from_reader_with_options(cursor, opts, |de| {
                Simple::deserialize(de)
            })
            .unwrap();
        assert_eq!(result, Simple { x: 11 });
    }
}

use serde_saphyr::{ArcAnchor, ArcRecursive, ArcRecursion, ArcWeakAnchor, RcAnchor, RcRecursive, RcRecursion, RcWeakAnchor};
use std::borrow::Borrow;
use std::sync::Arc;
use std::rc::Rc;

mod anchors_tests {
    use super::*;

    #[test]
    fn arc_anchor_wrapping_deref_asref_borrow_from_into() {
        let arc = Arc::new("hello".to_string());
        let anch1: ArcAnchor<String> = ArcAnchor::wrapping("world".to_string());
        let anch2: ArcAnchor<String> = arc.clone().into();
        assert_eq!(&**anch1, "world");
        assert_eq!(&**anch2, "hello");
        let _deref: &Arc<String> = &anch2;
        let _asref: &Arc<String> = anch2.as_ref();
        let _borrow: &Arc<String> = Borrow::borrow(&anch2);
        let back: Arc<String> = anch2.into();
        assert!(Arc::ptr_eq(&back, &arc));
    }

    #[test]
    fn arc_anchor_default_debug() {
        let anch: ArcAnchor<()> = ArcAnchor::default();
        let _dbg = format!("{:?}", anch);
    }

    #[test]
    fn rc_anchor_default_debug() {
        let anch: RcAnchor<()> = RcAnchor::default();
        let _dbg = format!("{:?}", anch);
    }

    #[test]
    fn arc_weak_anchor_from_strong_anchor() {
        let strong = Arc::new("hi".to_string());
        let weak1: ArcWeakAnchor<String> = strong.clone().into();
        let anch = ArcAnchor::wrapping("hi".to_string());
        let weak3: ArcWeakAnchor<String> = (&anch).into();
        drop(strong);
        drop(anch);
        assert!(weak1.upgrade().is_none());
        assert!(weak1.is_dangling());
        assert!(weak3.upgrade().is_none());
        assert!(weak3.is_dangling());
    }

    #[test]
    fn rc_weak_anchor_from_strong_anchor() {
        let strong = Rc::new("hi".to_string());
        let weak1: RcWeakAnchor<String> = strong.clone().into();
        let anch = RcAnchor::wrapping("hi".to_string());
        let weak3: RcWeakAnchor<String> = (&anch).into();
        drop(strong);
        drop(anch);
        assert!(weak1.upgrade().is_none());
        assert!(weak1.is_dangling());
        assert!(weak3.upgrade().is_none());
        assert!(weak3.is_dangling());
    }

    #[test]
    fn recursion_weak_from_strong_dangling() {
        let rec_strong_rc = RcRecursive::wrapping("rc".to_string());
        let rec_weak_rc: RcRecursion<String> = (&rec_strong_rc).into();
        drop(rec_strong_rc);
        assert!(rec_weak_rc.is_dangling());
        assert!(rec_weak_rc.upgrade().is_none());

        let rec_strong_arc = ArcRecursive::wrapping("arc".to_string());
        let rec_weak_arc: ArcRecursion<String> = (&rec_strong_arc).into();
        drop(rec_strong_arc);
        assert!(rec_weak_arc.is_dangling());
        assert!(rec_weak_arc.upgrade().is_none());
    }
}