#![no_main]

use libfuzzer_sys::fuzz_target;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug)]
struct DuplicateDoc {
    a: Option<i64>,
    key: Option<String>,
    outer: Option<Outer>,
}

#[derive(Deserialize, Debug)]
struct Outer {
    inner: Option<Inner>,
    arr: Option<Vec<Nested>>, 
}

#[derive(Deserialize, Debug)]
struct Inner {
    x: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct Nested {
    k: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DuplicateFlow {
    #[serde(flatten)]
    entries: BTreeMap<String, i64>,
}

// This fuzzer constructs YAML mappings with intentional duplicate keys to
// exercise duplicate-key strategies and diagnostics. It targets both Value
// and typed maps/struct-like shapes.
fuzz_target!(|data: &[u8]| {
    if data.len() > 16 * 1024 { return; }
    let s = String::from_utf8_lossy(data);

    // Simple top-level map with duplicates.
    let yaml_top = format!("a: 1\na: 2\nkey: {s}\nkey: {s}\n");

    // Nested duplicates within flow and block styles.
    let yaml_nested = format!(
        "outer:\n  inner: {{x: 1, x: 2}}\n  arr: [{{k: {s}}}, {{k: {s}}}]\n"
    );

    for y in [&yaml_top, &yaml_nested] {
        let _v = serde_saphyr::from_str::<DuplicateDoc>(y);
    }

    // Additionally, build YAML via flow mapping using the bytes as a key.
    let yaml_flow = format!("{{'{s}': 1, '{s}': 2}}\n");
    let _v_flow = serde_saphyr::from_str::<DuplicateFlow>(&yaml_flow);
});
