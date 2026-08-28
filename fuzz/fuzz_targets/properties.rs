#![no_main]

use std::collections::HashMap;
use std::io::Cursor;

use libfuzzer_sys::fuzz_target;
use serde::Deserialize;
use serde_saphyr::{Options, PropertySyntax};

/// Minimal deserialization target used to exercise property expansion through the public API.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Config {
    value: String,
}

fn options(syntax: PropertySyntax) -> Options {
    let properties = HashMap::from([
        ("SET".to_owned(), "value".to_owned()),
        ("EMPTY".to_owned(), String::new()),
    ]);
    serde_saphyr::options! {
        property_syntax: syntax,
        budget: serde_saphyr::budget! {
            max_property_expansion_depth: 64,
            max_total_property_interpolation_work: 1_048_576,
        },
    }
    .with_properties(properties)
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 64 * 1024 {
        return;
    }

    let syntax = if data.first().is_some_and(|byte| byte & 1 == 1) {
        PropertySyntax::BracedOrBare
    } else {
        PropertySyntax::Braced
    };

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = serde_saphyr::from_str_with_options::<String>(text, options(syntax));
        let _ =
            serde_saphyr::from_reader_with_options::<_, String>(Cursor::new(data), options(syntax));
    }

    let depth = data.get(1).map_or(0, |byte| usize::from(*byte));
    let operator = match data.get(2).copied().unwrap_or_default() % 6 {
        0 => "${MISSING-",
        1 => "${MISSING:-",
        2 => "${SET+",
        3 => "${SET:+",
        4 => "${MISSING?",
        _ => "${EMPTY:?",
    };
    let scalar = format!("{}value{}", operator.repeat(depth), "}".repeat(depth));
    let yaml = format!("value: {scalar}\n");

    let _ = serde_saphyr::from_str_with_options::<Config>(&yaml, options(syntax));
});
