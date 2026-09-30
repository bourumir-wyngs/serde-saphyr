//! CodSpeed / divan benchmarks for serde-saphyr.
//!
//! Run locally with `cargo bench --bench yaml`, or with CodSpeed via
//! `cargo codspeed build && cargo codspeed run`.

use divan::{Bencher, black_box};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

fn main() {
    divan::main();
}

#[derive(Debug, Deserialize, Serialize)]
struct Document {
    defaults: Defaults,
    items: Vec<Item>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Defaults {
    enabled: bool,
    roles: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Item {
    enabled: bool,
    roles: Vec<String>,
    id: usize,
    name: String,
    details: Details,
}

#[derive(Debug, Deserialize, Serialize)]
struct Details {
    description: String,
    notes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Service {
    name: String,
    image: String,
    replicas: u32,
    ports: Vec<u16>,
    env: BTreeMap<String, String>,
    cpu_limit: f64,
    enabled: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    version: String,
    services: Vec<Service>,
}

const SIZES: &[usize] = &[10, 100];

/// Options that keep the default budget but allow many aliases per anchor,
/// which the alias-heavy benchmark inputs intentionally use.
fn alias_heavy_options() -> serde_saphyr::Options {
    serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            enforce_alias_anchor_ratio: false,
        },
    }
}

/// Builds a YAML document shaped like `examples/benchmark.rs`, with anchors
/// and aliases shared by every item.
fn build_items_yaml(items: usize) -> String {
    let mut yaml = String::new();
    yaml.push_str("---\n");
    yaml.push_str("defaults:\n");
    yaml.push_str("  enabled: &defaults_enabled true\n");
    yaml.push_str("  roles: &defaults_roles\n");
    yaml.push_str("    - reader\n");
    yaml.push_str("    - writer\n");
    yaml.push_str("items:\n");
    for index in 0..items {
        let _ = write!(
            yaml,
            "  - enabled: *defaults_enabled\n    roles: *defaults_roles\n    id: {index}\n    name: item_{index:05}\n    details:\n      description: \"Item number {index:05} includes repeated notes for benchmarking performance.\"\n      notes:\n"
        );
        for note_index in 0..10 {
            let _ = writeln!(
                yaml,
                "        - \"Note {note_index:02} for item {index:05}. This is repeated content.\""
            );
        }
    }
    yaml
}

fn build_config(services: usize) -> Config {
    Config {
        version: "3.9".to_string(),
        services: (0..services)
            .map(|i| Service {
                name: format!("service-{i}"),
                image: format!("registry.example.com/team/app-{i}:1.{i}.0"),
                replicas: (i % 5) as u32 + 1,
                ports: vec![8000 + (i % 1000) as u16, 9000 + (i % 1000) as u16],
                env: (0..5)
                    .map(|k| (format!("VAR_{k}"), format!("value with spaces {i}-{k}")))
                    .collect(),
                cpu_limit: 0.25 * ((i % 8) as f64 + 1.0),
                enabled: i % 3 != 0,
            })
            .collect(),
    }
}

fn build_config_yaml(services: usize) -> String {
    serde_saphyr::to_string(&build_config(services)).expect("serialize config")
}

fn build_merge_keys_yaml(entries: usize) -> String {
    let mut yaml = String::from("base: &base\n  timeout: 30\n  retries: 3\n  region: eu-west-1\n");
    yaml.push_str("entries:\n");
    for i in 0..entries {
        let _ = write!(
            yaml,
            "  - <<: *base\n    name: entry-{i}\n    retries: {}\n",
            i % 7
        );
    }
    yaml
}

fn build_multi_doc_yaml(docs: usize) -> String {
    let mut yaml = String::new();
    for i in 0..docs {
        let _ = write!(
            yaml,
            "---\nname: service-{i}\nimage: app:{i}\nreplicas: {}\nports: [80, 443]\nenv: {{A: \"1\", B: two}}\ncpu_limit: 0.5\nenabled: true\n",
            i % 4 + 1
        );
    }
    yaml
}

mod deserialize {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn typed_with_aliases(bencher: Bencher, items: usize) {
        let yaml = build_items_yaml(items);
        bencher.bench(|| {
            let doc: Document =
                serde_saphyr::from_str_with_options(black_box(&yaml), alias_heavy_options())
                    .unwrap();
            doc
        });
    }

    #[divan::bench(args = SIZES)]
    fn typed_from_slice(bencher: Bencher, items: usize) {
        let yaml = build_items_yaml(items);
        bencher.bench(|| {
            let doc: Document = serde_saphyr::from_slice_with_options(
                black_box(yaml.as_bytes()),
                alias_heavy_options(),
            )
            .unwrap();
            doc
        });
    }

    #[divan::bench(args = SIZES)]
    fn typed_from_reader(bencher: Bencher, items: usize) {
        let yaml = build_items_yaml(items);
        bencher.bench(|| {
            let doc: Document = serde_saphyr::from_reader_with_options(
                black_box(yaml.as_bytes()),
                alias_heavy_options(),
            )
            .unwrap();
            doc
        });
    }

    #[divan::bench(args = SIZES)]
    fn config(bencher: Bencher, services: usize) {
        let yaml = build_config_yaml(services);
        bencher.bench(|| {
            let cfg: Config = serde_saphyr::from_str(black_box(&yaml)).unwrap();
            cfg
        });
    }

    #[divan::bench(args = SIZES)]
    fn untyped_json_value(bencher: Bencher, services: usize) {
        let yaml = build_config_yaml(services);
        bencher.bench(|| {
            let value: serde_json::Value = serde_saphyr::from_str(black_box(&yaml)).unwrap();
            value
        });
    }

    #[divan::bench(args = SIZES)]
    fn merge_keys(bencher: Bencher, entries: usize) {
        let yaml = build_merge_keys_yaml(entries);
        bencher.bench(|| {
            let value: serde_json::Value =
                serde_saphyr::from_str_with_options(black_box(&yaml), alias_heavy_options())
                    .unwrap();
            value
        });
    }

    #[divan::bench(args = SIZES)]
    fn multi_document(bencher: Bencher, docs: usize) {
        let yaml = build_multi_doc_yaml(docs);
        bencher.bench(|| {
            let services: Vec<Service> = serde_saphyr::from_str_multiple(black_box(&yaml)).unwrap();
            services
        });
    }
}

mod serialize {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn config(bencher: Bencher, services: usize) {
        let cfg = build_config(services);
        bencher.bench(|| serde_saphyr::to_string(black_box(&cfg)).unwrap());
    }

    #[divan::bench(args = SIZES)]
    fn document(bencher: Bencher, items: usize) {
        let yaml = build_items_yaml(items);
        let doc: Document =
            serde_saphyr::from_str_with_options(&yaml, alias_heavy_options()).unwrap();
        bencher.bench(|| serde_saphyr::to_string(black_box(&doc)).unwrap());
    }

    #[divan::bench(args = SIZES)]
    fn multi_document(bencher: Bencher, docs: usize) {
        let services = build_config(docs).services;
        bencher.bench(|| serde_saphyr::to_string_multiple(black_box(&services)).unwrap());
    }
}

#[divan::bench(args = SIZES)]
fn round_trip(bencher: Bencher, services: usize) {
    let cfg = build_config(services);
    bencher.bench(|| {
        let yaml = serde_saphyr::to_string(black_box(&cfg)).unwrap();
        let back: Config = serde_saphyr::from_str(&yaml).unwrap();
        back
    });
}
