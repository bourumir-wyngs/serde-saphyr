#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use serde::Deserialize;

// This fuzzer focuses on flow-style collections: sequences [..] and mappings {..}.
// It wraps the fuzzer input into flow collections in several ways and exercises
// deserialization through different entry points.

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
fuzz_target!(|data: &[u8]| {
    if data.len() > 16 * 1024 { return; }

    let s = String::from_utf8_lossy(data);

    // 1) Flow sequence
    let yaml_seq = format!("[{s}]");
    // 2) Flow mapping
    let yaml_map = format!("{{{s}}}");
    // 3) A struct-like document using flow mapping at top level
    let yaml_doc = format!("root: {{{s}}}\narray: [{s}]\n");

    for (idx, y) in [&yaml_seq, &yaml_map, &yaml_doc].into_iter().enumerate() {
        match idx {
            0 => {
                let _ = serde_saphyr::from_str::<FlowSeq>(y);
            }
            1 => {
                let _ = serde_saphyr::from_str::<FlowMap>(y);
            }
            _ => {
                let _ = serde_saphyr::from_str::<Doc>(y);
            }
        }
    }
});
