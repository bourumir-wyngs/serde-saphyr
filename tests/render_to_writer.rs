#![cfg(feature = "deserialize")]

use std::{borrow::Cow, fmt};

use rstest::rstest;
use serde_saphyr::{Error, MessageFormatter, SnippetMode};

struct CustomMessage;

impl MessageFormatter for CustomMessage {
    fn format_message<'a>(&self, _error: &'a Error) -> Cow<'a, str> {
        Cow::Borrowed("custom error")
    }
}

#[rstest]
#[case(SnippetMode::Off)]
#[case(SnippetMode::Auto)]
fn render_to_appends_with_options(#[case] snippets: SnippetMode) {
    let error = serde_saphyr::from_str::<u8>("eighty").unwrap_err();
    let original_location = error.location();
    let options = serde_saphyr::render_options! {
        formatter: &CustomMessage,
        snippets: snippets,
        source_name: Some("config.yaml"),
        line_offset: 9,
    };
    let mut output = String::from("prefix: ");
    let writer: &mut dyn fmt::Write = &mut output;
    error.render_to(writer, options).unwrap();

    assert_eq!(
        output,
        format!("prefix: {}", error.render_with_options(options))
    );
    if snippets == SnippetMode::Off {
        assert_eq!(output, "prefix: custom error at line 10, column 1");
    } else {
        assert!(output.contains("custom error"), "{output}");
        assert!(output.contains("config.yaml:10:1"), "{output}");
        assert!(output.contains("10 | eighty"), "{output}");
    }
    assert_eq!(error.location(), original_location);
}

#[derive(Default)]
struct FailingWriter {
    calls: usize,
}

impl fmt::Write for FailingWriter {
    fn write_str(&mut self, _text: &str) -> fmt::Result {
        self.calls += 1;
        Err(fmt::Error)
    }
}

#[rstest]
#[case(SnippetMode::Off)]
#[case(SnippetMode::Auto)]
fn render_to_propagates_writer_errors(#[case] snippets: SnippetMode) {
    let error = serde_saphyr::from_str::<u8>("eighty").unwrap_err();
    let options = serde_saphyr::render_options! { snippets: snippets };
    let mut writer = FailingWriter::default();

    assert_eq!(error.render_to(&mut writer, options), Err(fmt::Error));
    assert_eq!(writer.calls, 1);
}
