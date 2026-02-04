//! Demonstrates customizing error rendering with a custom [`MessageFormatter`] and [`Localizer`].
//!
//! This example shows two layers of customization:
//!
//! 1. [`MessageFormatter::format_message`] for the *core* error message text (e.g. translating
//!    `Error::Eof` into pirate-speak).
//! 2. [`Localizer`] for message pieces that are composed *outside* `format_message`, such as:
//!    - location suffixes (the "… at line X, column Y" part)
//!    - validation issue lines (and validation snippet labels when snippets are enabled)
//!
//! The built-in formatters ([`UserMessageFormatter`] and [`DefaultMessageFormatter`]) can also be
//! reused while swapping their wording glue via `.with_localizer(...)`.
//!
//! Run:
//! ```bash
//! cargo run --example pirate_formatter
//! cargo run --example pirate_formatter --features garde
//! cargo run --example pirate_formatter --features validator
//! ```
//!
//! Notes:
//! - Snippets are enabled (`SnippetMode::Auto`). For validation errors, snippets are only shown
//!   when the error value is wrapped in `Error::WithSnippet`, which depends on the parsing
//!   options (see the validation examples below).

use serde_saphyr::{Error, Localizer, Location, MessageFormatter, UserMessageFormatter};
use serde::Deserialize;
use std::borrow::Cow;

/// Pirate wording for message pieces that are formatted *outside* `MessageFormatter::format_message`.
///
/// This includes:
/// - location suffixes ("at line X, column Y")
/// - validation issue lines ("validation error at ...")
struct PirateLocalizer;

impl Localizer for PirateLocalizer {
    fn attach_location<'a>(&self, base: Cow<'a, str>, loc: serde_saphyr::Location) -> Cow<'a, str> {
        if loc == serde_saphyr::Location::UNKNOWN {
            return base;
        }
        Cow::Owned(format!(
            "{base}. Bug lurks on line {}, then {} runes in",
            loc.line(),
            loc.column()
        ))
    }

    fn validation_issue_line(
        &self,
        resolved_path: &str,
        entry: &str,
        loc: Option<serde_saphyr::Location>,
    ) -> String {
        let base = format!("Arrr! Ye violated the code at {resolved_path}: {entry}");
        match loc {
            Some(l) if l != serde_saphyr::Location::UNKNOWN => {
                self.attach_location(Cow::Owned(base), l).into_owned()
            }
            _ => base,
        }
    }

    fn validation_base_message(&self, entry: &str, resolved_path: &str) -> String {
        // This text is shown as the snippet title/label for validation errors.
        // Keep it pirate-speak so validation snippets are fully localized.
        format!("Arrr! Ye violated the code: {entry} for `{resolved_path}`")
    }

    fn snippet_location_prefix(&self, loc: serde_saphyr::Location) -> String {
        if loc == serde_saphyr::Location::UNKNOWN {
            String::new()
        } else {
            format!("Bug lurks on line {}, then {} runes in", loc.line(), loc.column())
        }
    }

    fn value_used_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("This be where the scribble is put to use!")
    }

    fn value_comes_from_the_anchor(&self, def: Location) -> String {
        format!(
            "  | This scribble hails from the anchor at line {}, column {}:",
            def.line(), def.column()
        )
    }

    fn invalid_here(&self, base: &str) -> String {
        format!("crap here, {base}")
    }

    fn defined_window(&self) -> Cow<'static, str> {
        Cow::Borrowed("scribbled here")
    }
}

/// A custom formatter that translates error messages into Pirate speak.
struct PirateFormatter;

impl MessageFormatter for PirateFormatter {
    fn localizer(&self) -> &dyn Localizer {
        &PirateLocalizer
    }

    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        let default = UserMessageFormatter.with_localizer(&PirateLocalizer);
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
            // Localizer stays pirate-speak.
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
            e.render_with_formatter(&serde_saphyr::DEFAULT_MESSAGE_FORMATTER)
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
            e.render_with_formatter(&serde_saphyr::DEFAULT_MESSAGE_FORMATTER)
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
                e.render_with_formatter(&serde_saphyr::DEFAULT_MESSAGE_FORMATTER)
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

    // Example 3: Triggered when deserializing to `&str` but the scalar needs transformation (e.g. escapes).
    let yaml_transformed_str = "\"hello\\nworld\"\n";
    println!("\n\n--- Attempting to parse a transformed scalar into &str ---");
    println!("{}", yaml_transformed_str.trim());

    let result: Result<&str, _> = serde_saphyr::from_str(yaml_transformed_str);

    if let Err(e) = result {
        println!(
            "\n[Developer Error]:\n{}",
            e.render_with_formatter(&serde_saphyr::DEFAULT_MESSAGE_FORMATTER)
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

    // Example 4: Validation with dual-snippet (anchor + alias)
    // The invalid value is defined with an anchor and referenced via alias,
    // so the error shows both where the alias is used and where the anchor defined the value.
    #[cfg(feature = "garde")]
    {
        use garde::Validate;

        #[derive(Debug, Deserialize, Validate)]
        #[allow(dead_code)]
        struct Cfg {
            #[garde(skip)]
            name: String,
            #[garde(length(min = 2))]
            nickname: String,
        }

        // The anchor &short defines "x" (too short), and *short references it for nickname
        let yaml = "name: &short \"x\"\nnickname: *short\n";
        println!("\n\n--- Attempting to parse YAML with garde validation error (dual-snippet) ---");
        println!("{}", yaml.trim());

        let err = serde_saphyr::from_str_with_options_valid::<Cfg>(
            yaml,
            serde_saphyr::options! { with_snippet: true },
        )
        .expect_err("validation error expected");

        println!("\n[User Error]:\n{}", err.render_with_formatter(&UserMessageFormatter));
        println!("\n[Pirate Error]:\n{}", err.render_with_formatter(&PirateFormatter));
    }

    #[cfg(feature = "validator")]
    {
        use validator::Validate;

        #[derive(Debug, Deserialize, Validate)]
        #[allow(dead_code)]
        struct Cfg {
            #[validate(skip)]
            name: String,
            #[validate(length(min = 2))]
            nickname: String,
        }

        // The anchor &short defines "x" (too short), and *short references it for nickname
        let yaml = "name: &short \"x\"\nnickname: *short\n";
        println!("\n\n--- Attempting to parse YAML with validator validation error (dual-snippet) ---");
        println!("{}", yaml.trim());

        let err = serde_saphyr::from_str_with_options_validate::<Cfg>(
            yaml,
            serde_saphyr::options! { with_snippet: true },
        )
        .expect_err("validation error expected");

        println!("\n[User Error]:\n{}", err.render_with_formatter(&UserMessageFormatter));
        println!("\n[Pirate Error]:\n{}", err.render_with_formatter(&PirateFormatter));
    }
}
