#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![allow(
    dead_code,
    reason = "fuzz target helper structs are only used as deserialization shapes"
)]

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FlowSeq(#[serde(default)] Vec<String>);

#[derive(Debug, Deserialize)]
struct FlowMap(#[serde(default)] BTreeMap<String, String>);

#[derive(Debug, Deserialize)]
struct Doc {
    #[serde(default)]
    root: Option<FlowMap>,
    #[serde(default)]
    array: Option<FlowSeq>,
}

fn run_flow_collections_fuzzer_entrypoints(data: &[u8]) {
    if data.len() > 16 * 1024 {
        return;
    }

    let s = String::from_utf8_lossy(data);

    let yaml_seq = format!("[{s}]");
    let yaml_map = format!("{{{s}}}");
    let yaml_doc = format!("root: {{{s}}}\narray: [{s}]\n");

    let _ = serde_saphyr::from_str::<FlowSeq>(&yaml_seq);
    let _ = serde_saphyr::from_str::<FlowMap>(&yaml_map);
    let _ = serde_saphyr::from_str::<Doc>(&yaml_doc);
}

#[test]
fn case_1() {
    let data = [
        255u8, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 10, 9, 35, 8, 10, 9, 255,
        255, 255, 255, 255, 10, 9, 35, 8, 35, 91, 93, 58,
    ];

    run_flow_collections_fuzzer_entrypoints(&data);
}

#[test]
fn case_2() {
    let data = [134, 128, 128, 128, 128, 3, 42, 44, 198, 10, 9, 35, 10, 9, 41, 10,
        9, 35, 125, 46];

    run_flow_collections_fuzzer_entrypoints(&data);
}

