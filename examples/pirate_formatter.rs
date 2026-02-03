use serde_saphyr::{DefaultMessageFormatter, Error, MessageFormatter, UserMessageFormatter};
use serde::Deserialize;
use std::borrow::Cow;

/// A custom formatter that translates error messages into Pirate speak.
struct PirateFormatter;

impl MessageFormatter for PirateFormatter {
    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        let default = UserMessageFormatter;
        match err {
            Error::Eof { .. } => Cow::Borrowed("Arrr! The scroll be endin' too soon, matey!"),
            Error::MultipleDocuments { .. } => Cow::Borrowed(
                "Yer scroll be split into many tales, but we be expectin' just one!",
            ),
            Error::CannotBorrowTransformedString { .. } => Cow::Borrowed(
                "That string got mangled by the waves",
            ),
            Error::UnknownAnchor { .. } => {
                Cow::Borrowed("Mark be missing from the map!")
            }
            // For other errors, we can delegate to the user-facing formatter.
            _ => default.format_message(err),
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CrewMember {
    name: String
}

fn main() {
    let yaml_multiple_docs = "name: A\n---\nname: B\n";
    println!("--- Attempting to parse multiple YAML documents into a single value ---");
    println!("{}", yaml_multiple_docs.trim());

    let result: Result<CrewMember, _> = serde_saphyr::from_str(yaml_multiple_docs);

    if let Err(e) = result {
        println!(
            "\n[Developer Error]:\n{}",
            e.render_with_formatter(&DefaultMessageFormatter)
        );
        println!(
            "\n[User Error]:\n{}",
            e.render_with_formatter(&UserMessageFormatter)
        );
        println!(
            "\n[Pirate Error]:\n{}",
            e.render_with_formatter(&PirateFormatter)
        );
    }

    // Example 1: Unknown Anchor
    let yaml_anchor = "
- rum
- *dead_mans_chest
";
    println!("\n\n--- Attempting to parse YAML with unknown anchor ---");
    println!("{}", yaml_anchor.trim());

    let result: Result<Vec<String>, _> = serde_saphyr::from_str(yaml_anchor);

    if let Err(e) = result {
        println!(
            "\n[Developer Error]:\n{}",
            e.render_with_formatter(&DefaultMessageFormatter)
        );
        println!(
            "\n[User Error]:\n{}",
            e.render_with_formatter(&UserMessageFormatter)
        );
        println!(
            "\n[Pirate Error]:\n{}",
            e.render_with_formatter(&PirateFormatter)
        );
    }

    // Example 2: EOF
    let yaml_eof = "";
    println!("\n\n--- Attempting to parse empty YAML into String ---");
    
    let result: Result<String, _> = serde_saphyr::from_str(yaml_eof);
    
    match result {
        Ok(_) => println!("Surprise! It parsed successfully (which is unexpected for empty input -> String)."),
        Err(e) => {
            println!(
                "\n[Developer Error]:\n{}",
                e.render_with_formatter(&DefaultMessageFormatter)
            );
            println!(
                "\n[User Error]:\n{}",
                e.render_with_formatter(&UserMessageFormatter)
            );
            println!(
                "\n[Pirate Error]:\n{}",
                e.render_with_formatter(&PirateFormatter)
            );
        }
    }

    // Triggered when deserializing to `&str` but the scalar needs transformation (e.g. escapes).
    let yaml_transformed_str = "\"hello\\nworld\"\n";
    println!("\n\n--- Attempting to parse a transformed scalar into &str ---");
    println!("{}", yaml_transformed_str.trim());

    let result: Result<&str, _> = serde_saphyr::from_str(yaml_transformed_str);

    if let Err(e) = result {
        println!(
            "\n[Developer Error]:\n{}",
            e.render_with_formatter(&DefaultMessageFormatter)
        );
        println!(
            "\n[User Error]:\n{}",
            e.render_with_formatter(&UserMessageFormatter)
        );
        println!(
            "\n[Pirate Error]:\n{}",
            e.render_with_formatter(&PirateFormatter)
        );
    }
}
