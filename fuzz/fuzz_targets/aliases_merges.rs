#![no_main]

use libfuzzer_sys::fuzz_target;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AliasDoc {
    a: Option<String>,
    b: Option<String>,
    seq: Option<Vec<i64>>,
    #[serde(rename = "seq_alias")]
    seq_alias: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
struct MergeDoc {
    base1: Option<BaseMap>,
    base2: Option<BaseMap>,
    merged: Option<MergedMap>,
}

#[derive(Debug, Deserialize)]
struct BaseMap {
    k: Option<i64>,
    v: Option<String>,
    w: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MergedMap {
    #[serde(rename = "<<")]
    merges: Option<Vec<BaseMap>>,
    extra: Option<i64>,
    k: Option<i64>,
    v: Option<String>,
    w: Option<String>,
}

// This fuzzer biases inputs toward anchors, aliases, and YAML merge keys (<<).
// It constructs a couple of YAML documents influenced by the input and runs
// both Value-preserving and normal deserialization paths.
fuzz_target!(|data: &[u8]| {
    if data.len() > 16 * 1024 { return; }

    // Basic bytes to string; replace invalid UTF-8 with replacement so we can embed it safely.
    let s = String::from_utf8_lossy(data);

    // 1) Anchors and aliases only.
    let yaml_alias = format!(
        "a: &A {s}\nb: *A\nseq: &S [1, 2, 3]\nseq_alias: *S\n"
    );

    // 2) Merge keys: build two maps that get merged into target.
    let yaml_merge = format!(
        "base1: &B1 {{k: 1, v: {s}}}\nbase2: &B2 {{k: 2, w: {s}}}\nmerged: {{<<: [*B1, *B2], extra: 3}}\n"
    );

    let _alias: Result<AliasDoc, _> = serde_saphyr::from_str(&yaml_alias);
    let _merge: Result<MergeDoc, _> = serde_saphyr::from_str(&yaml_merge);
});
