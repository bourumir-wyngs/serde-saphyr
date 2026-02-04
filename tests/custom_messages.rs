use serde_saphyr::{Error, MessageFormatter, from_str};
use serde_saphyr::from_slice;
use serde_saphyr::UserMessageFormatter;

use std::borrow::Cow;

#[test]
fn test_user_formatter_eof() {
    let err = from_str::<String>("").unwrap_err();
    let msg = err.render_with_formatter(&UserMessageFormatter);
    assert!(msg.contains("unexpected end of file"));
}

#[test]
fn test_user_formatter_unknown_anchor() {
    // Real-world repro: unknown alias/anchor in YAML.
    let err = from_str::<String>("*missing").unwrap_err();

    let user_msg = err.render_with_formatter(&UserMessageFormatter);
    assert!(user_msg.contains("reference to unknown value"));
    assert!(!user_msg.contains("id"));
}

#[test]
fn test_user_formatter_quoting_required() {
    // To trigger quoting required: deserialize into string with no_schema/special chars?
    // Or just construct it if possible. Error variants are public.
    // But constructors are private.
    // Error::QuotingRequired is public.
    
    let err = Error::QuotingRequired { 
        value: "foo:bar".to_string(), 
        location: serde_saphyr::Location::UNKNOWN 
    };
    
    let default_msg = err.to_string();
    assert!(default_msg.contains("The string value [foo:bar] must be quoted"));
    
    let user_msg = err.render_with_formatter(&UserMessageFormatter);
    assert_eq!(user_msg, "value requires quoting");
}

#[test]
fn invalid_utf8_input_from_slice_mentions_utf8_not_binary() {
    // Invalid UTF-8 (0xFF is never a valid leading byte).
    let bytes = [0xFFu8, b'\n'];

    let err = from_slice::<String>(&bytes).unwrap_err();
    assert!(matches!(err, Error::InvalidUtf8Input));

    let default_msg = err.to_string();
    assert!(default_msg.to_ascii_lowercase().contains("utf-8"));
    assert!(!default_msg.contains("!!binary"));

    let user_msg = err.render_with_formatter(&UserMessageFormatter);
    assert!(user_msg.to_ascii_lowercase().contains("utf-8"));
    assert!(!user_msg.contains("!!binary"));
}

#[cfg(feature = "garde")]
#[test]
fn custom_formatter_is_used_for_nested_validation_errors_with_snippets() {
    struct Custom;
    impl MessageFormatter for Custom {
        fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
            match err {
                Error::UnknownAnchor { .. } => Cow::Borrowed("custom unknown anchor"),
                // Keep the header empty so the assertion only targets nested errors.
                Error::ValidationErrors { .. } => Cow::Borrowed(""),
                _ => UserMessageFormatter.format_message(err),
            }
        }
    }

    let yaml = "*missing\n";
    let loc = from_str::<String>(yaml).unwrap_err().location().unwrap();
    let nested = Error::UnknownAnchor { location: loc };

    let err = Error::WithSnippet {
        text: yaml.to_string(),
        start_line: 1,
        crop_radius: 2,
        error: Box::new(Error::ValidationErrors {
            errors: vec![nested],
        }),
    };

    let out = err.render_with_formatter(&Custom);
    assert!(out.contains("custom unknown anchor"));
}

#[cfg(feature = "validator")]
#[test]
fn custom_formatter_is_used_for_nested_validator_errors_with_snippets() {
    struct Custom;
    impl MessageFormatter for Custom {
        fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
            match err {
                Error::UnknownAnchor { .. } => Cow::Borrowed("custom unknown anchor"),
                // Keep the header empty so the assertion only targets nested errors.
                Error::ValidatorErrors { .. } => Cow::Borrowed(""),
                _ => UserMessageFormatter.format_message(err),
            }
        }
    }

    let yaml = "*missing\n";
    let loc = from_str::<String>(yaml).unwrap_err().location().unwrap();
    let nested = Error::UnknownAnchor { location: loc };

    let err = Error::WithSnippet {
        text: yaml.to_string(),
        start_line: 1,
        crop_radius: 2,
        error: Box::new(Error::ValidatorErrors {
            errors: vec![nested],
        }),
    };

    let out = err.render_with_formatter(&Custom);
    assert!(out.contains("custom unknown anchor"));
}

#[cfg(feature = "miette")]
#[test]
fn test_miette_integration() {
    use serde_saphyr::miette::to_miette_report_with_formatter;
    
    let yaml = "*unknown";
    let err = Error::UnknownAnchor {
        location: serde_saphyr::Location::UNKNOWN,
    };
    
    let report = to_miette_report_with_formatter(&err, yaml, "test.yaml", &UserMessageFormatter);
    let out = report.to_string();
    println!("Miette out: {}", out);
    
    assert!(out.contains("reference to unknown value"));
    assert!(!out.contains("id unknown"));
}
