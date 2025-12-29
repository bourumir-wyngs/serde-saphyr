use indoc::indoc;
use serde::Deserialize;

use serde_saphyr::Spanned;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Dev,
    Prod,
}

#[derive(Debug, Deserialize)]
struct ServerCfg {
    host: Spanned<String>,
    port: Spanned<u16>,
}

#[derive(Debug, Deserialize)]
struct NestedCfg {
    threshold: Spanned<f64>,
    flags: Vec<Spanned<String>>,
    inner: InnerCfg,
}

#[derive(Debug, Deserialize)]
struct InnerCfg {
    enabled: Spanned<bool>,
    note: Spanned<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct Cfg {
    name: Spanned<String>,
    timeout: Spanned<u64>,
    mode: Spanned<Mode>,
    servers: Vec<ServerCfg>,
    vals: Vec<Spanned<u64>>,
    nested: NestedCfg,
}

#[test]
fn spanned_captures_scalar_value_locations() {
    let yaml = indoc! {"# top comment
name: bar # app name
timeout: 5
mode: prod
servers: [ # short list
  { host: a.example, port: 8080 }, # first
  { host: b.example, port: 9090 } # second
]
vals: [10, 20] # flow seq
nested: { threshold: 0.25, flags: [fast, safe], inner: { enabled: true, note: null } } # end
"};

    let cfg: Cfg = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(cfg.name.value, "bar");
    assert_eq!(cfg.name.referenced.line(), 2);
    assert_eq!(cfg.name.referenced.column(), 7);
    assert_eq!(cfg.name.defined, cfg.name.referenced);

    assert_eq!(cfg.timeout.value, 5);
    assert_eq!(cfg.timeout.referenced.line(), 3);
    assert_eq!(cfg.timeout.referenced.column(), 10);
    assert_eq!(cfg.timeout.defined, cfg.timeout.referenced);

    assert_eq!(cfg.mode.value, Mode::Prod);
    assert_eq!(cfg.mode.referenced.line(), 4);
    assert_eq!(cfg.mode.referenced.column(), 7);
    assert_eq!(cfg.mode.defined, cfg.mode.referenced);

    assert_eq!(cfg.servers.len(), 2);
    assert_eq!(cfg.servers[0].host.value, "a.example");
    assert_eq!(cfg.servers[0].host.referenced.line(), 6);
    assert_eq!(cfg.servers[0].host.referenced.column(), 11);
    assert_eq!(cfg.servers[0].port.value, 8080);
    assert_eq!(cfg.servers[0].port.referenced.line(), 6);
    assert_eq!(cfg.servers[0].port.referenced.column(), 28);

    assert_eq!(cfg.servers[1].host.value, "b.example");
    assert_eq!(cfg.servers[1].host.referenced.line(), 7);
    assert_eq!(cfg.servers[1].host.referenced.column(), 11);
    assert_eq!(cfg.servers[1].port.value, 9090);
    assert_eq!(cfg.servers[1].port.referenced.line(), 7);
    assert_eq!(cfg.servers[1].port.referenced.column(), 28);

    assert_eq!(cfg.vals.len(), 2);
    assert_eq!(cfg.vals[0].value, 10);
    assert_eq!(cfg.vals[0].referenced.line(), 9);
    assert_eq!(cfg.vals[0].referenced.column(), 8);

    assert_eq!(cfg.vals[1].value, 20);
    assert_eq!(cfg.vals[1].referenced.line(), 9);
    assert_eq!(cfg.vals[1].referenced.column(), 12);

    assert!((cfg.nested.threshold.value - 0.25).abs() < 1e-12);
    assert_eq!(cfg.nested.threshold.referenced.line(), 10);
    assert_eq!(cfg.nested.threshold.referenced.column(), 22);

    assert_eq!(cfg.nested.flags.len(), 2);
    assert_eq!(cfg.nested.flags[0].value, "fast");
    assert_eq!(cfg.nested.flags[0].referenced.line(), 10);
    assert_eq!(cfg.nested.flags[0].referenced.column(), 36);
    assert_eq!(cfg.nested.flags[1].value, "safe");
    assert_eq!(cfg.nested.flags[1].referenced.line(), 10);
    assert_eq!(cfg.nested.flags[1].referenced.column(), 42);

    assert_eq!(cfg.nested.inner.enabled.value, true);
    assert_eq!(cfg.nested.inner.enabled.referenced.line(), 10);
    assert_eq!(cfg.nested.inner.enabled.referenced.column(), 67);

    assert_eq!(cfg.nested.inner.note.value, None);
    assert_eq!(cfg.nested.inner.note.referenced.line(), 10);
    assert_eq!(cfg.nested.inner.note.referenced.column(), 79);
}

#[test]
fn spanned_captures_locations_in_block_sequences() {
    let yaml = indoc! {"name: bar
timeout: 5
mode: prod
servers:
  - host: a.example
    port: 8080
  - host: b.example
    port: 9090
vals:
  - 10
  - 20
nested:
  threshold: 0.25
  flags:
    - fast
    - safe
  inner:
    enabled: true
    note: null
"};

    let cfg: Cfg = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(cfg.name.value, "bar");
    assert_eq!(cfg.name.referenced.line(), 1);
    assert_eq!(cfg.name.referenced.column(), 7);

    assert_eq!(cfg.timeout.value, 5);
    assert_eq!(cfg.timeout.referenced.line(), 2);
    assert_eq!(cfg.timeout.referenced.column(), 10);

    assert_eq!(cfg.mode.value, Mode::Prod);
    assert_eq!(cfg.mode.referenced.line(), 3);
    assert_eq!(cfg.mode.referenced.column(), 7);

    assert_eq!(cfg.servers.len(), 2);
    assert_eq!(cfg.servers[0].host.value, "a.example");
    assert_eq!(cfg.servers[0].host.referenced.line(), 5);
    assert_eq!(cfg.servers[0].host.referenced.column(), 11);
    assert_eq!(cfg.servers[0].port.value, 8080);
    assert_eq!(cfg.servers[0].port.referenced.line(), 6);
    assert_eq!(cfg.servers[0].port.referenced.column(), 11);

    assert_eq!(cfg.servers[1].host.value, "b.example");
    assert_eq!(cfg.servers[1].host.referenced.line(), 7);
    assert_eq!(cfg.servers[1].host.referenced.column(), 11);
    assert_eq!(cfg.servers[1].port.value, 9090);
    assert_eq!(cfg.servers[1].port.referenced.line(), 8);
    assert_eq!(cfg.servers[1].port.referenced.column(), 11);

    assert_eq!(cfg.vals.len(), 2);
    assert_eq!(cfg.vals[0].value, 10);
    assert_eq!(cfg.vals[0].referenced.line(), 10);
    assert_eq!(cfg.vals[0].referenced.column(), 5);
    assert_eq!(cfg.vals[1].value, 20);
    assert_eq!(cfg.vals[1].referenced.line(), 11);
    assert_eq!(cfg.vals[1].referenced.column(), 5);

    assert!((cfg.nested.threshold.value - 0.25).abs() < 1e-12);
    assert_eq!(cfg.nested.threshold.referenced.line(), 13);
    assert_eq!(cfg.nested.threshold.referenced.column(), 14);

    assert_eq!(cfg.nested.flags.len(), 2);
    assert_eq!(cfg.nested.flags[0].value, "fast");
    assert_eq!(cfg.nested.flags[0].referenced.line(), 15);
    assert_eq!(cfg.nested.flags[0].referenced.column(), 7);
    assert_eq!(cfg.nested.flags[1].value, "safe");
    assert_eq!(cfg.nested.flags[1].referenced.line(), 16);
    assert_eq!(cfg.nested.flags[1].referenced.column(), 7);

    assert_eq!(cfg.nested.inner.enabled.value, true);
    assert_eq!(cfg.nested.inner.enabled.referenced.line(), 18);
    assert_eq!(cfg.nested.inner.enabled.referenced.column(), 14);

    assert_eq!(cfg.nested.inner.note.value, None);
    assert_eq!(cfg.nested.inner.note.referenced.line(), 19);
    assert_eq!(cfg.nested.inner.note.referenced.column(), 11);
}

#[derive(Debug, Deserialize)]
struct AliasMergeCfg {
    x: Spanned<u64>,
    merged_host: Spanned<String>,
}

#[test]
fn spanned_tracks_referenced_and_defined_for_alias_and_merge() {
    let yaml = indoc! {"base_scalar: &a 123
x: *a

base_map: &m
  merged_host: example.com

<<: *m
"};

    let cfg: AliasMergeCfg = serde_saphyr::from_str(yaml).unwrap();

    // x references *a but is defined at the anchor.
    assert_eq!(cfg.x.value, 123);
    assert_eq!(cfg.x.referenced.line(), 2);
    assert_eq!(cfg.x.referenced.column(), 4);
    assert_eq!(cfg.x.defined.line(), 1);
    assert_eq!(cfg.x.defined.column(), 17);

    // merged_host comes from merge; referenced at the merge entry, defined at the source scalar.
    assert_eq!(cfg.merged_host.value, "example.com");
    assert_eq!(cfg.merged_host.referenced.line(), 7);
    assert_eq!(cfg.merged_host.referenced.column(), 5);
    assert_eq!(cfg.merged_host.defined.line(), 5);
    assert_eq!(cfg.merged_host.defined.column(), 16);
}

/// KEMN = Key Empty Map Node
#[derive(Debug, Deserialize)]
struct KeyEmptyMapNoneAliasCfg {
    m: std::collections::BTreeMap<Option<String>, Spanned<u64>>,
}

#[test]
fn spanned_preserves_use_site_for_alias_values_in_kemn_slow_path() {
    // This YAML uses a complex mapping key whose shape triggers the “KEMN one-entry nullish”
    // slow-path in `deserialize_map`.
    //
    // The value is also an alias (`*a`). When the value is captured as a node, the captured
    // events come from the anchor definition (definition-site), but `Spanned<T>.referenced`
    // must point at the alias token (use-site).
    let yaml = indoc! {"base: &a 123
m:
  ? { null: *a }
  : *a
"};

    let cfg: KeyEmptyMapNoneAliasCfg = serde_saphyr::from_str(yaml).unwrap();
    let v = cfg.m.get(&None).expect("expected a None key entry");

    assert_eq!(v.value, 123);

    // Use-site: the alias token on line 4 (`: *a`).
    assert_eq!(v.referenced.line(), 4);
    assert_eq!(v.referenced.column(), 5);

    // Definition-site: where the anchored scalar is defined (`&a 123`).
    assert_eq!(v.defined.line(), 1);
    assert_eq!(v.defined.column(), 10);
}

#[derive(Debug, Deserialize)]
struct SeqMergeCfg {
    a: Spanned<String>,
    b: Spanned<String>,
}

#[test]
fn spanned_tracks_use_site_per_element_for_sequence_merges() {
    let yaml = indoc! {"base1: &m1
  a: A
base2: &m2
  b: B
<<: [*m1, *m2]
"};

    let cfg: SeqMergeCfg = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(cfg.a.value, "A");
    assert_eq!(cfg.a.referenced.line(), 5);
    assert_eq!(cfg.a.referenced.column(), 6); // *m1
    assert_eq!(cfg.a.defined.line(), 2);
    assert_eq!(cfg.a.defined.column(), 6); // "  a: A"

    assert_eq!(cfg.b.value, "B");
    assert_eq!(cfg.b.referenced.line(), 5);
    assert_eq!(cfg.b.referenced.column(), 11); // *m2
    assert_eq!(cfg.b.defined.line(), 4);
    assert_eq!(cfg.b.defined.column(), 6); // "  b: B"
}
