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

    /// Ordinary plain strings reject the YAML 1.1 boolean probe. Strict mode
    /// uses the YAML 1.2 probe instead, providing a control for error allocation.
    #[divan::bench(args = [false, true])]
    fn untyped_strings(bencher: Bencher, strict_booleans: bool) {
        let yaml: String = (0..64).map(|i| format!("- service-{i}\n")).collect();
        let options = || serde_saphyr::options! { strict_booleans: strict_booleans };
        let parsed: serde_json::Value =
            serde_saphyr::from_str_with_options(&yaml, options()).unwrap();
        let values = parsed.as_array().unwrap();
        assert_eq!(values.len(), 64);
        assert!(values.iter().all(serde_json::Value::is_string));

        bencher.bench(|| {
            serde_saphyr::from_str_with_options::<serde_json::Value>(black_box(&yaml), options())
                .unwrap()
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

/// Focused workloads for scalar scanning and quoted output. Build fixtures outside
/// the timed closure; each benchmark still exercises the public serializer API.
mod strings {
    use super::*;
    use serde_saphyr::{DoubleQuoted, SingleQuoted};

    const KINDS: &[&str] = &["ascii", "unicode", "sparse_escapes", "dense_escapes"];

    fn text(kind: &str) -> String {
        match kind {
            "ascii" => "ordinary text with spaces ".repeat(64),
            "unicode" => "Žąsis 日本語 🦀 café ".repeat(64),
            "sparse_escapes" => {
                "a long ordinary stretch of text before a quote: \" and slash: \\ ".repeat(32)
            }
            "dense_escapes" => "\"\\\t\n\0\u{85}\u{2028}".repeat(128),
            _ => unreachable!(),
        }
    }

    #[divan::bench(args = KINDS)]
    fn double_quoted(bencher: Bencher, kind: &str) {
        let value = DoubleQuoted(text(kind));
        bencher.bench(|| serde_saphyr::to_string(black_box(&value)).unwrap());
    }

    #[divan::bench(args = KINDS)]
    fn double_quoted_io(bencher: Bencher, kind: &str) {
        let value = DoubleQuoted(text(kind));
        bencher.bench(|| {
            let mut out = Vec::new();
            serde_saphyr::to_io_writer(&mut out, black_box(&value)).unwrap();
            out
        });
    }

    #[divan::bench(args = ["ascii", "unicode", "apostrophes"])]
    fn single_quoted(bencher: Bencher, kind: &str) {
        let value = SingleQuoted(single_quoted_text(kind));
        bencher.bench(|| serde_saphyr::to_string(black_box(&value)).unwrap());
    }

    fn single_quoted_text(kind: &str) -> String {
        if kind == "apostrophes" {
            "it's a quoted string with an apostrophe ".repeat(64)
        } else {
            text(kind)
        }
    }

    /// Exercise batched literal writes through the fmt-to-I/O adapter as well.
    #[divan::bench(args = ["ascii", "unicode", "apostrophes"])]
    fn single_quoted_io(bencher: Bencher, kind: &str) {
        let value = SingleQuoted(single_quoted_text(kind));
        bencher.bench(|| {
            let mut out = Vec::new();
            serde_saphyr::to_io_writer(&mut out, black_box(&value)).unwrap();
            out
        });
    }

    /// Isolate automatic folding from YAML 1.1 boolean probes. Unicode cases
    /// have more bytes than the width, so they still need a character count.
    #[divan::bench(args = [
        "short_ascii", "at_width_ascii", "over_width_ascii",
        "short_unicode", "at_width_unicode", "over_width_unicode",
    ])]
    fn automatic_folding(bencher: Bencher, kind: &str) {
        let (unit, count) = match kind {
            "short_ascii" => ("a", 32),
            "at_width_ascii" => ("a", 80),
            "over_width_ascii" => ("a", 81),
            "short_unicode" => ("é", 48),
            "at_width_unicode" => ("é", 80),
            "over_width_unicode" => ("é", 81),
            _ => unreachable!(),
        };
        let value = unit.repeat(count);
        let options = || {
            serde_saphyr::ser_options! {
                yaml_12: true,
                no_lang_directive: true,
                folded_wrap_chars: 80,
            }
        };
        // Guard the intended path outside the measurement, including the case
        // where byte length exceeds the width but character count does not.
        let yaml = serde_saphyr::to_string_with_options(&value, options()).unwrap();
        assert_eq!(yaml.contains(">-\n"), count > 80);
        bencher
            .bench(|| serde_saphyr::to_string_with_options(black_box(&value), options()).unwrap());
    }

    #[divan::bench(args = ["ascii", "unicode", "hashes", "comment"])]
    fn plain(bencher: Bencher, kind: &str) {
        let value = match kind {
            "hashes" => "path#fragment ".repeat(128),
            "comment" => "ordinary text with spaces ".repeat(64) + "\u{2003}#comment",
            _ => text(kind),
        };
        // Keep these scalars plain; trailing spaces would benchmark quoting instead.
        let value = value.trim_end();
        bencher.bench(|| {
            serde_saphyr::to_string_with_options(
                black_box(&value),
                serde_saphyr::ser_options! { prefer_block_scalars: false },
            )
            .unwrap()
        });
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
