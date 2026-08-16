use granit_parser::{Event, Parser, ScalarStyle, Tag};
use serde::de::IgnoredAny;
use serde_json::Value;
use std::borrow::Cow;

pub(super) fn assert_json_case(yaml: &str, expected_json: &str) {
    let expected: Vec<Value> = serde_json::Deserializer::from_str(expected_json)
        .into_iter::<Value>()
        .collect::<Result<_, _>>()
        .expect("yaml-test-suite JSON must be valid");

    let actual = if expected.len() == 1 {
        vec![
            serde_saphyr::from_str::<Value>(yaml)
                .expect("YAML must deserialize as one JSON-compatible document"),
        ]
    } else {
        serde_saphyr::from_multiple::<Value>(yaml)
            .expect("YAML stream must deserialize as JSON-compatible documents")
    };

    assert_eq!(actual, expected);
}

pub(super) fn assert_invalid_case(yaml: &str) {
    let raw_result = Parser::new_from_str(yaml).collect::<Result<Vec<_>, _>>();
    assert!(
        raw_result.is_err(),
        "granit-parser unexpectedly accepted invalid YAML"
    );

    match serde_saphyr::from_multiple::<IgnoredAny>(yaml) {
        Err(_) => {}
        Ok(documents) => panic!(
            "serde_saphyr unexpectedly accepted invalid YAML as {} document(s)",
            documents.len()
        ),
    }
}

pub(super) fn assert_valid_events(yaml: &str, expected_events: &str) {
    assert_valid_events_with_options(yaml, expected_events, serde_saphyr::Options::default());
}

pub(super) fn assert_valid_events_with_options(
    yaml: &str,
    expected_events: &str,
    options: serde_saphyr::Options,
) {
    serde_saphyr::from_multiple_with_options::<IgnoredAny>(yaml, options)
        .expect("event-only YAML case must be valid through serde_saphyr");

    let actual = Parser::new_from_str(yaml)
        .map(|item| {
            let (event, _) =
                item.expect("event-only YAML case must be valid through granit-parser");
            event
        })
        .filter_map(render_event)
        .collect::<Vec<_>>();
    let expected = normalize_expected_events(expected_events);

    assert_eq!(actual, expected);
}

fn render_event(event: Event<'_>) -> Option<String> {
    let rendered = match event {
        Event::StreamStart => "+STR".to_owned(),
        Event::StreamEnd => "-STR".to_owned(),
        Event::DocumentStart(..) => "+DOC".to_owned(),
        Event::DocumentEnd => "-DOC".to_owned(),
        Event::SequenceStart(_, anchor, tag) => {
            format!("+SEQ{}{}", format_anchor(anchor), format_tag(tag.as_ref()))
        }
        Event::SequenceEnd => "-SEQ".to_owned(),
        Event::MappingStart(_, anchor, tag) => {
            format!("+MAP{}{}", format_anchor(anchor), format_tag(tag.as_ref()))
        }
        Event::MappingEnd => "-MAP".to_owned(),
        Event::Scalar(text, style, anchor, tag) => {
            let style = match style {
                ScalarStyle::Plain => ":",
                ScalarStyle::SingleQuoted => "'",
                ScalarStyle::DoubleQuoted => "\"",
                ScalarStyle::Literal => "|",
                ScalarStyle::Folded => ">",
            };
            format!(
                "=VAL{}{} {style}{}",
                format_anchor(anchor),
                format_tag(tag.as_ref()),
                escape_text(&text)
            )
        }
        Event::Alias(anchor) => format!("=ALI *{anchor}"),
        Event::Comment(..) => return None,
        _ => return None,
    };

    Some(rendered)
}

fn format_anchor(anchor: usize) -> String {
    if anchor == 0 {
        String::new()
    } else {
        format!(" &{anchor}")
    }
}

fn format_tag(tag: Option<&Cow<'_, Tag>>) -> String {
    match tag {
        Some(tag) => format!(" <{}{}>", tag.handle(), tag.suffix()),
        None => String::new(),
    }
}

fn escape_text(text: &str) -> String {
    let mut escaped = text.to_owned();
    for (character, replacement) in [
        ('\\', r"\\"),
        ('\n', "\\n"),
        ('\r', "\\r"),
        ('\x08', "\\b"),
        ('\t', "\\t"),
    ] {
        escaped = escaped.replace(character, replacement);
    }
    escaped
}

fn normalize_expected_events(expected: &str) -> Vec<String> {
    let mut anchors = Vec::new();

    expected
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut line = line.to_owned();

            if let Some(start) = line.find('&')
                && !line[..start].contains(':')
            {
                let length = line[start..]
                    .find(' ')
                    .unwrap_or_else(|| line[start..].len());
                anchors.push(line[start + 1..start + length].to_owned());
                line.replace_range(start..start + length, &format!("&{}", anchors.len()));
            }

            if line.starts_with("=ALI") {
                let start = line.find('*').expect("alias event must name its anchor");
                let name = &line[start + 1..];
                let index = anchors
                    .iter()
                    .rposition(|anchor| anchor == name)
                    .expect("alias event must refer to an earlier anchor");
                line.replace_range(start.., &format!("*{}", index + 1));
            }

            match line.as_str() {
                "+DOC ---" => "+DOC".to_owned(),
                "-DOC ..." => "-DOC".to_owned(),
                line if line.starts_with("+SEQ []") => line.replacen("+SEQ []", "+SEQ", 1),
                line if line.starts_with("+MAP {}") => line.replacen("+MAP {}", "+MAP", 1),
                "=VAL :" => "=VAL :~".to_owned(),
                line if line.starts_with("=VAL &")
                    && !line.contains('<')
                    && line.ends_with(" :") =>
                {
                    format!("{line}~")
                }
                line => line.to_owned(),
            }
        })
        .collect()
}
