#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::borrow::Cow;

mod localizer_tests {
    use super::*;
    use serde_saphyr::Location;
    use serde_saphyr::localizer::{
        DEFAULT_ENGLISH_LOCALIZER, DefaultEnglishLocalizer, ExternalMessage, ExternalMessageSource,
        Localizer,
    };

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
        let s = DEFAULT_ENGLISH_LOCALIZER.validation_issue_line("x", "y", Some(Location::UNKNOWN));
        assert!(!s.contains("at line"));
    }

    #[test]
    fn validation_issue_line_known_location() {
        let err = serde_saphyr::from_str::<u8>("not-a-number").unwrap_err();
        let loc = err.location().expect("parse error should have a location");
        let s =
            DEFAULT_ENGLISH_LOCALIZER.validation_issue_line("root.value", "bad value", Some(loc));
        assert!(s.contains("validation error at root.value: bad value"));
        assert!(s.contains("line"), "expected location suffix, got: {s}");
    }

    #[test]
    fn join_validation_issues() {
        let lines = vec!["a".into(), "b".into()];
        assert_eq!(
            DEFAULT_ENGLISH_LOCALIZER.join_validation_issues(&lines),
            "a\nb"
        );
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
        let msg = ExternalMessage::new(
            ExternalMessageSource::Parser(serde_saphyr::granit_parser::ScanError::new(
                serde_saphyr::granit_parser::Marker::new(0, 1, 0),
                "scan error",
            )),
            "scan error",
        );
        assert!(
            DEFAULT_ENGLISH_LOCALIZER
                .override_external_message(msg)
                .is_none()
        );
    }

    #[test]
    fn external_message_builder_sets_code() {
        let msg = ExternalMessage::new(
            ExternalMessageSource::Parser(serde_saphyr::granit_parser::ScanError::new(
                serde_saphyr::granit_parser::Marker::new(0, 1, 0),
                "scan error",
            )),
            "scan error",
        )
        .with_code("invalid_yaml");

        assert_eq!(msg.code, Some("invalid_yaml"));
    }

    #[test]
    fn default_english_localizer_is_debug_clone_copy() {
        let l = DefaultEnglishLocalizer;
        let _ = format!("{:?}", l);
        let l2 = l;
        let _ = l2;
    }

    /// Custom localizer that overrides one method.
    #[derive(Debug, Clone, Copy)]
    struct SpanishLocalizer;
    impl Localizer for SpanishLocalizer {
        fn root_path_label(&self) -> Cow<'static, str> {
            Cow::Borrowed("<raíz>")
        }
        fn defined(&self) -> Cow<'static, str> {
            Cow::Borrowed("(definido)")
        }
    }

    #[test]
    fn custom_localizer_override() {
        let l = SpanishLocalizer;
        assert_eq!(l.root_path_label(), "<raíz>");
        assert_eq!(l.defined(), "(definido)");
        // Other methods still return English defaults
        assert_eq!(l.defined_here(), "(defined here)");

        let _ = format!("{:?}", l);
        let l2 = l;
        let _ = l2;
    }
}
