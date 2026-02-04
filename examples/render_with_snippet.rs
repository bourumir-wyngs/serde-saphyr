//! Demonstrates snippet rendering for YAML deserialization errors.
//!
//! # Single-location errors
//! Most errors have a single location and render with one snippet showing
//! where the error occurred.
//!
//! # Dual-snippet errors (anchor + alias)
//! When an error occurs while deserializing an aliased value, and the alias
//! usage site differs from the anchor definition site, the renderer shows
//! TWO snippets:
//! - Primary snippet: "the value is used here" - pointing to alias usage
//! - Secondary snippet: "defined here" - pointing to anchor definition
//!
//! This helps users understand that an error at an alias usage site originated
//! from a value defined elsewhere in the YAML document.

use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Cfg {
    base_scalar: serde_saphyr::Spanned<u64>,
    key: Vec<usize>,
}

fn main() {
    println!("=== Single-location error (standard snippet) ===\n");

    // Intentionally invalid YAML to demonstrate snippet rendering.
    let yaml = r#"
    base_scalar: -z123 # this should be a number
    key: [ 1, 2, 2 ]
"#;

    let cfg: Result<Cfg, Error> = serde_saphyr::from_str(yaml);
    match cfg {
        Ok(cfg) => println!("{:?}", cfg),
        Err(err) => eprintln!("{err}"),
    }

    println!("\n=== Dual-snippet error (anchor + alias) ===\n");

    // This example demonstrates dual-snippet rendering when an error occurs
    // at an alias usage site. The anchor defines a valid value for one field,
    // but when the alias is used for a field expecting a different type, the
    // error shows BOTH locations.

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Config {
        // First field expects u64 - anchor value 42 works here
        count: u64,
        // Second field expects bool - alias *val fails here
        flag: bool,
    }

    // YAML with anchor and alias - the anchor value (42) is valid for `count`,
    // but when *val is used for `flag` (bool), it fails at the alias site.
    let yaml_with_alias = r#"
count: &val 42
flag: *val
"#;

    println!("Parsing YAML with anchor (&val) and alias (*val):");
    println!("{}", yaml_with_alias);

    let result: Result<Config, Error> = serde_saphyr::from_str(yaml_with_alias);
    match result {
        Ok(cfg) => println!("Parsed: {:?}", cfg),
        Err(err) => {
            eprintln!("{err}");

            // Show the error structure
            println!("\n--- Error details ---");
            if let Some(loc) = err.location() {
                println!(
                    "Primary location: line {}, column {}",
                    loc.line(),
                    loc.column()
                );
            }
            if let Some(locs) = err.locations() {
                println!(
                    "Reference location: line {}, column {}",
                    locs.reference_location.line(),
                    locs.reference_location.column()
                );
                println!(
                    "Defined location: line {}, column {}",
                    locs.defined_location.line(),
                    locs.defined_location.column()
                );
                if locs.reference_location != locs.defined_location {
                    println!("  -> Locations differ: dual-snippet rendering applied!");
                } else {
                    println!("  -> Locations are the same: single snippet rendered");
                }
            }
        }
    }

    println!("\n=== How dual-snippet rendering works ===\n");
    println!("When an error occurs during alias replay and the reference (alias) location");
    println!("differs from the defined (anchor) location, the error renderer:");
    println!("  1. Shows the primary snippet at the alias usage site");
    println!("  2. Shows a secondary snippet at the anchor definition site");
    println!("  3. Labels them appropriately ('the value is used here' / 'defined here')");
}
