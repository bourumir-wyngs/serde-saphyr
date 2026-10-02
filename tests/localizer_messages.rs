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

mod alias_error_tests {
    use super::*;
    use serde::Deserialize;
    use serde_saphyr::localizer::Localizer;
    use serde_saphyr::{DefaultMessageFormatter, Error, Location};

    #[derive(Debug, Deserialize)]
    struct Config {
        #[allow(dead_code)]
        name: String,
        #[allow(dead_code)]
        port: u16,
    }

    struct Bracketed;

    impl Localizer for Bracketed {
        fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
            Cow::Owned(format!("{base} [{}:{}]", loc.line(), loc.column()))
        }
    }

    /// An error inside an aliased value keeps the inner error (issue #199), so a custom
    /// localizer formats the alias location and `source()` returns the inner error.
    #[test]
    fn alias_error_keeps_the_inner_error() {
        let yaml = "name: &n eighty\nport: *n\n";
        let outer = serde_saphyr::from_str::<Config>(yaml).unwrap_err();
        let error = outer.without_snippet();

        let Error::AliasError {
            error: inner,
            locations,
            ..
        } = error
        else {
            panic!("expected Error::AliasError, got {error:?}");
        };
        let reference = locations.reference_location;
        let defined = locations.defined_location;
        assert_eq!((reference.line(), reference.column()), (2, 7));
        assert_eq!((defined.line(), defined.column()), (1, 10));

        let source = std::error::Error::source(error).expect("source() returns the inner error");
        let source = source
            .downcast_ref::<Error>()
            .expect("the source retains its concrete Error type");
        assert!(matches!(source, Error::InvalidScalar { ty: "u16", .. }));
        assert!(std::ptr::eq(source, inner.as_ref()));
        assert_eq!(source.to_string(), inner.to_string());

        // Existing handlers that inspect the legacy message still match alias errors.
        #[allow(deprecated)]
        match error {
            Error::AliasError { msg, .. } => assert_eq!(msg, &inner.to_string()),
            other => panic!("the existing AliasError handler must run, got {other:?}"),
        }

        assert_eq!(
            error.to_string(),
            "invalid u16 (defined at line 1, column 10) at line 2, column 7",
            "the anchor definition and alias use must each be reported once"
        );

        let rendered =
            error.render_with_formatter(&DefaultMessageFormatter.with_localizer(&Bracketed));
        assert_eq!(rendered, "invalid u16 (defined at line 1, column 10) [2:7]");
    }
}
