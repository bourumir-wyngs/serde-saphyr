#![allow(unused_imports)]
use serde_saphyr::{DefaultMessageFormatter, Error, MessageFormatter, UserMessageFormatter};

#[cfg(feature = "garde")]
use garde::Validate;
#[cfg(feature = "validator")]
use validator::Validate as ValidatorValidate;
use serde::Deserialize;
use std::borrow::Cow;

/// A custom formatter that translates error messages into Pirate speak.
struct PirateFormatter;

impl MessageFormatter for PirateFormatter {
    fn format_message(&self, err: &Error) -> String {
        let msg: Cow<'_, str> = match err {
            Error::Eof { .. } => Cow::Borrowed("Arrr! The scroll be endin' too soon, matey!"),
            Error::MultipleDocuments { .. } => Cow::Borrowed(
                "Yer scroll be split into many tales, but we be expectin' just one!",
            ),
            Error::CannotBorrowTransformedString { .. } => Cow::Borrowed(
                "That string got mangled by the waves — ye can't borrow it as-is. Use a String or Cow<str>, savvy?",
            ),
            Error::Unexpected { expected, .. } => Cow::Owned(format!(
                "Shiver me timbers! We expected {expected}, but found somethin' else!"
            )),
            Error::UnknownAnchor { id, .. } => {
                Cow::Owned(format!("Anchor {id} be missing from the map!"))
            }
            Error::AliasError { .. } => {
                Cow::Borrowed("That map reference leads to Davey Jones' locker!")
            }
            Error::Budget { .. } => {
                Cow::Borrowed("We're out of powder, too many shells fired. Avast!")
            }
            Error::QuotingRequired { .. } => {
                Cow::Borrowed("Ye need to cage that beast in quotes, savvy?")
            }
            Error::ContainerEndMismatch { .. } => {
                Cow::Borrowed("Ye opened a chest but didn't close it properly!")
            }
            Error::Message { msg, .. } => Cow::Owned(format!("Ahoy! {msg}")),
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => Cow::Owned(format!(
                "Arrr! Garde found {} scallywags in yer papers!",
                errors.len()
            )),
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => Cow::Owned(format!(
                "Arrr! Validator found {} scallywags in yer papers!",
                errors.len()
            )),
            #[cfg(feature = "validator")]
            Error::ValidatorError { .. } => {
                Cow::Borrowed("Validator says, this crew member ain't fit for duty!")
            }
            #[cfg(feature = "garde")]
            Error::ValidationError { .. } => {
                Cow::Borrowed("Garde says, this crew member ain't fit for duty!")
            }

            // For other errors, we can delegate to the default formatter or just print the debug
            _ => Cow::Owned(format!("Yarrr! Something be wrong: {err:?}")),
        };
        msg.into_owned()
    }
}

#[cfg(feature = "validator")]
#[derive(Debug, Deserialize, ValidatorValidate)]
struct CrewMember {
    #[validate(length(min = 3))]
    name: String,
    #[validate(range(min = 18))]
    age: u8,
}

#[cfg(not(feature = "validator"))]
#[derive(Debug, Deserialize)]
struct CrewMember {
    name: String,
    age: u8,
}

fn main() {
    // Example 0: Multiple Documents
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
- item1
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

    // Example 2: Unexpected Type
    let yaml_unexpected = "
- item1
- [nested, list]
";
    println!("\n\n--- Attempting to parse YAML list into Vec<String> ---");
    println!("{}", yaml_unexpected.trim());

    // We expect a list of strings, but the second item is a list
    let result: Result<Vec<String>, _> = serde_saphyr::from_str(yaml_unexpected);

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
    
    // Example 3: EOF
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

    // Example 4: CannotBorrowTransformedString
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

    // Example 5: Alias Error (Type Mismatch on Alias)
    // To trigger AliasError, we need an alias to a value that fails deserialization.
    // e.g. alias points to a string, but we expect an integer.
    let yaml_alias_error = "
anchor: &a string_value
value: *a
";
    println!("\n\n--- Attempting to parse Alias Error (Type Mismatch) ---");
    println!("{}", yaml_alias_error.trim());
    
    #[derive(Deserialize)]
    struct Config {
        #[allow(dead_code)]
        anchor: String,
        #[allow(dead_code)]
        value: i32, // Expecting int, but getting "string_value" via alias
    }

    let result: Result<Config, _> = serde_saphyr::from_str(yaml_alias_error);
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

    // Example 5: Validation Errors (if feature enabled)
    #[cfg(feature = "validator")]
    {
        println!("\n\n--- Attempting to parse Invalid Crew Members (Validator) ---");
        
        let yaml_invalid_crew = "
name: Jo
age: 10
"; 
        println!("{}", yaml_invalid_crew.trim());
        
        // Use `from_str_validate` to enable validation during deserialization
        let result: Result<CrewMember, _> = serde_saphyr::from_str_validate(yaml_invalid_crew);
        
        match result {
            Ok(_) => println!("Validation passed unexpectedly!"),
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
        
        // Example 6: Multiple Documents Validation Errors
        println!("\n\n--- Attempting to parse Multiple Invalid Crew Members (Validator) ---");
        let yaml_multiple_invalid = "
name: Jo
age: 10
---
name: ValidName
age: 5
---
name: Mo
age: 100
";
        println!("{}", yaml_multiple_invalid.trim());
        
        let result: Result<Vec<CrewMember>, _> = serde_saphyr::from_multiple_validate(yaml_multiple_invalid);
        match result {
             Ok(_) => println!("All crew members passed validation unexpectedly!"),
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
    }
}
