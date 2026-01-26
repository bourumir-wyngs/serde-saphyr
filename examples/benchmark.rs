//! Benchmarking test. This is used periodically during development to make sure
//! we do not degrade the performance when working on features.

use serde::Deserialize;
use serde_saphyr::Error;
use serde_saphyr::budget::Budget;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct Document {
    defaults: Defaults,
    items: Vec<Item>,
}

#[derive(Debug, Deserialize, Clone)]
struct Defaults {
    enabled: bool,
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Item {
    enabled: bool,
    roles: Vec<String>,
    id: usize,
    name: String,
    details: Details,
}

#[derive(Debug, Deserialize)]
struct Details {
    description: String,
    notes: Vec<String>,
}

fn build_large_yaml(target_size: usize) -> String {
    let mut yaml = String::with_capacity(target_size + 1024);
    yaml.push_str("---\n");
    yaml.push_str("defaults:\n");
    yaml.push_str("  enabled: &defaults_enabled true\n");
    yaml.push_str("  roles: &defaults_roles\n");
    yaml.push_str("    - reader\n");
    yaml.push_str("    - writer\n");
    yaml.push_str("items:\n");

    let mut index = 0usize;
    while yaml.len() < target_size {
        let mut entry = format!(
            "  - enabled: *defaults_enabled\n    roles: *defaults_roles\n    id: {index}\n    name: item_{index:05}\n    details:\n      description: \"Item number {index:05} includes repeated notes for benchmarking performance.\"\n      notes:\n"
        );

        for note_index in 0..20 {
            entry.push_str(&format!(
                "        - \"Note {note_index:02} for item {index:05}. This is repeated content to enlarge the YAML payload size considerably.\"\n"
            ));
        }

        yaml.push_str(&entry);
        index += 1;
    }

    yaml
}

fn main() -> Result<(), Error> {
    let target_size = 25 * 1024 * 1024; // 25 MiB
    let yaml = build_large_yaml(target_size);

    println!(
        "Generated YAML size: {:.2} MiB ({} bytes)",
        yaml.len() as f64 / (1024.0 * 1024.0),
        yaml.len()
    );

    // Use an effectively-unlimited budget while staying portable to 32-bit targets (e.g. wasm32).
    let many: usize = usize::MAX;
    let start = Instant::now();
    let document: Document = serde_saphyr::from_str_with_options(
        &yaml,
        serde_saphyr::options! {
            budget: Some(Budget {
                max_reader_input_bytes: None,
                max_events: many,
                max_aliases: many,
                max_anchors: many,
                max_depth: many,
                max_documents: many,
                max_nodes: many,
                max_total_scalar_bytes: many,
                max_merge_keys: many,
                enforce_alias_anchor_ratio: false,
                alias_anchor_min_aliases: many,
                alias_anchor_ratio_multiplier: many,
            }),
        },
    )?;
    let elapsed = start.elapsed();

    let total_notes: usize = document
        .items
        .iter()
        .map(|item| item.details.notes.len())
        .sum();
    let enabled_items = document.items.iter().filter(|item| item.enabled).count();
    let combined_roles: usize = document.items.iter().map(|item| item.roles.len()).sum();
    let name_checksum: usize = document.items.iter().map(|item| item.name.len()).sum();
    let max_id = document.items.iter().map(|item| item.id).max().unwrap_or(0);
    let description_bytes: usize = document
        .items
        .iter()
        .map(|item| item.details.description.len())
        .sum();

    println!("Parsed {} items in {:.2?}", document.items.len(), elapsed);
    println!(
        "Defaults enabled: {} (roles: {}), enabled items: {}, combined roles references: {}",
        document.defaults.enabled,
        document.defaults.roles.len(),
        enabled_items,
        combined_roles
    );
    println!(
        "Total notes: {} (name checksum: {}, max id: {}, description bytes: {})",
        total_notes, name_checksum, max_id, description_bytes
    );

    Ok(())
}
