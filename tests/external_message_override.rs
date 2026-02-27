#[cfg(any(feature = "garde", feature = "validator"))]
use serde::Deserialize;
use serde_saphyr::localizer::{ExternalMessage, ExternalMessageSource, Localizer};
use serde_saphyr::{DefaultMessageFormatter, Error, Location};
use std::borrow::Cow;

struct OverrideAllExternal;

impl Localizer for OverrideAllExternal {
    fn override_external_message<'a>(&self, msg: ExternalMessage<'a>) -> Option<Cow<'a, str>> {
        match msg.source {
            ExternalMessageSource::SaphyrParser => Some(Cow::Borrowed("OVERRIDDEN_PARSER")),
            ExternalMessageSource::Garde => Some(Cow::Borrowed("OVERRIDDEN_GARDE")),
            ExternalMessageSource::Validator => Some(Cow::Borrowed("OVERRIDDEN_VALIDATOR")),
        }
    }

    fn attach_location<'a>(&self, base: Cow<'a, str>, _loc: Location) -> Cow<'a, str> {
        // Keep assertions stable by not appending a location suffix.
        base
    }
}

#[test]
fn scan_error_text_can_be_overridden() {
    // Trigger a parser scan error.
    let err = serde_saphyr::from_str::<Vec<String>>(" [1, 2\n 3, 4 aaaaaa")
        .expect_err("scan error expected");

    let fmt = DefaultMessageFormatter.with_localizer(&OverrideAllExternal);
    let rendered = err.render_with_formatter(&fmt);

    assert!(rendered.contains("OVERRIDDEN_PARSER"));
}

#[cfg(feature = "validator")]
#[test]
fn validator_issue_text_can_be_overridden() {
    use validator::Validate;

    #[derive(Debug, Deserialize, Validate)]
    #[allow(dead_code)]
    struct Cfg {
        #[validate(skip)]
        name: String,
        #[validate(length(min = 2))]
        nickname: String,
    }

    let yaml = "name: &short \"x\"\nnickname: *short\n";
    let err = serde_saphyr::from_str_with_options_validate::<Cfg>(
        yaml,
        serde_saphyr::options! { with_snippet: true },
    )
    .expect_err("validation error expected");

    let fmt = DefaultMessageFormatter.with_localizer(&OverrideAllExternal);
    let rendered = err.render_with_formatter(&fmt);

    assert!(rendered.contains("OVERRIDDEN_VALIDATOR"));
}

#[cfg(feature = "garde")]
#[test]
fn garde_issue_text_can_be_overridden() {
    use garde::Validate;

    #[derive(Debug, Deserialize, Validate)]
    #[allow(dead_code)]
    struct Cfg {
        #[garde(skip)]
        name: String,
        #[garde(length(min = 2))]
        nickname: String,
    }

    let yaml = "name: &short \"x\"\nnickname: *short\n";
    let err = serde_saphyr::from_str_with_options_valid::<Cfg>(
        yaml,
        serde_saphyr::options! { with_snippet: true },
    )
    .expect_err("validation error expected");

    let fmt = DefaultMessageFormatter.with_localizer(&OverrideAllExternal);
    let rendered = err.render_with_formatter(&fmt);

    assert!(rendered.contains("OVERRIDDEN_GARDE"));
}

// Compile-time guard: make sure our new variant exists and is public.
#[test]
fn external_message_variant_is_public() {
    let err = Error::ExternalMessage {
        source: ExternalMessageSource::SaphyrParser,
        msg: "x".to_owned(),
        code: None,
        params: Vec::new(),
        location: Location::UNKNOWN,
    };
    let _ = err.to_string();
}
