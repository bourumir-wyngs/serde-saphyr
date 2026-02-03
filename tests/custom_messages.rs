use serde_saphyr::{Error, from_str};
use serde_saphyr::UserMessageFormatter;

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

#[cfg(feature = "miette")]
#[test]
fn test_miette_integration() {
    use serde_saphyr::miette::to_miette_report_with_formatter;
    
    let yaml = "*unknown";
    let err = Error::UnknownAnchor {
        id: 1,
        location: serde_saphyr::Location::UNKNOWN,
    };
    
    let report = to_miette_report_with_formatter(&err, yaml, "test.yaml", &UserMessageFormatter);
    let out = report.to_string();
    println!("Miette out: {}", out);
    
    assert!(out.contains("reference to unknown value"));
    assert!(!out.contains("id unknown"));
}
