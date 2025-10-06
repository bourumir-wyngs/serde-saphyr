use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
mod utils;

fn collect_test_inputs(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut inputs = Vec::new();
    if !base.exists() {
        return Ok(inputs);
    }
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
           inputs.push(path);
        }
    }
    Ok(inputs)
}

// Parse all documents from raw bytes using serde_yaml.
// Important: YAML requires Unicode. serde_yaml will reject inputs that are not
// valid UTF‑8 or not valid YAML. We use this behavior as the reference: if
// serde_yaml errors (including on non‑UTF‑8), we treat the test input as
// unsupported and skip any assertions against our parser for that file.
fn parse_all_with_serde_yaml_from_bytes(input: &[u8]) -> anyhow::Result<Vec<serde_yaml::Value>> {
    let mut docs = Vec::new();
    let des = serde_yaml::Deserializer::from_slice(input);
    for doc in des {
        let v = serde_yaml::Value::deserialize(doc)?;
        docs.push(v);
    }
    Ok(docs)
}

fn parse_all_with_bw_from_bytes(input: &[u8]) -> serde_saphyr::Result<Vec<serde_saphyr::Value>> {
    use serde::Deserialize;
    use serde_saphyr::{Deserializer, Value};
    let opts = utils::opts_no_pathology();
    let mut out = Vec::new();
    let de = Deserializer::from_slice_with_options(input, &opts);
    for doc in de {
        let v = Value::deserialize(doc)?;
        out.push(v);
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Ok,
    Error,
    Crash,
}

// Run serde_yaml under catch_unwind to classify outcomes without aborting the test.
// Status::Ok yields the parsed documents; Status::Error covers both parse errors
// and the "zero documents" case (which we also treat as an error for differential
// testing); Status::Crash captures panics inside the library.
fn classify_serde(input: &[u8]) -> (Status, Option<Vec<serde_yaml::Value>>) {
    let res = catch_unwind(AssertUnwindSafe(|| parse_all_with_serde_yaml_from_bytes(input)));
    match res {
        Err(_) => (Status::Crash, None),
        Ok(Err(_)) => (Status::Error, None),
        Ok(Ok(v)) if v.is_empty() => (Status::Error, None),
        Ok(Ok(v)) => (Status::Ok, Some(v)),
    }
}

// Same classifier for our parser, also protected with catch_unwind so we can
// print a clean per-file summary even if our code panics on some inputs.
fn classify_bw(input: &[u8]) -> (Status, Option<Vec<serde_saphyr::Value>>) {
    let res = catch_unwind(AssertUnwindSafe(|| parse_all_with_bw_from_bytes(input)));
    match res {
        Err(_) => (Status::Crash, None),
        Ok(Err(_)) => (Status::Error, None),
        Ok(Ok(v)) if v.is_empty() => (Status::Error, None),
        Ok(Ok(v)) => (Status::Ok, Some(v)),
    }
}

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Error => "error",
        Status::Crash => "crash",
    }
}

// Differential test over the fuzz_crashes corpus.
// Strategy:
// 1) Load file as raw bytes (may be non‑UTF‑8).
// 2) Let serde_yaml parse it; if serde_yaml rejects or yields zero docs, we print
//    a per-file summary and skip (the input is outside YAML or not Unicode).
// 3) Otherwise, our parser must succeed (no panic, no error). We print a summary
//    and then perform two roundtrip checks using serde_yaml as the semantic oracle:
//    (a) our parse -> our serialize -> serde_yaml parse equals serde_yaml docs
//    (b) serde_yaml docs -> our serialize -> our parse -> our serialize -> serde_yaml parse equals original serde_yaml docs.
#[test]
fn yaml_test_suite_differential() -> Result<()> {
    let base = Path::new("tests/fuzz_crashes");
    if !base.exists() {
        eprintln!("tests/fuzz_crashes directory not found; skipping differential test");
        return Ok(());
    }

    let inputs = collect_test_inputs(base)?;
    if inputs.is_empty() {
        eprintln!("No files found. Skipping differential test");
        return Ok(());
    }

    let mut tested = 0usize;
    let mut skipped = 0usize;

    for file in inputs {
        let yaml_bytes = fs::read(&file)
            .with_context(|| format!("reading {}", file.display()))?;

        // First, classify serde_yaml outcome and capture docs if successful.
        let (serde_status, ser_docs_opt) = classify_serde(&yaml_bytes);
        if serde_status != Status::Ok {
            // Also classify our parser for summary purposes, but do not assert.
            let (bw_status, _bw_docs_opt) = classify_bw(&yaml_bytes);
            eprintln!(
                "summary: {} | ours: {} | serde_yaml: {}",
                file.display(), status_str(bw_status), status_str(serde_status)
            );
            skipped += 1;
            continue;
        }
        let ser_docs = ser_docs_opt.expect("status Ok must include docs");

        // Our parser must be able to parse if serde_yaml did. Catch crashes to report nicely.
        let bw_attempt = catch_unwind(AssertUnwindSafe(|| parse_all_with_bw_from_bytes(&yaml_bytes)));
        let bw_docs = match bw_attempt {
            Err(_) => {
                eprintln!(
                    "summary: {} | ours: crash | serde_yaml: ok",
                    file.display()
                );
                let yaml_preview = String::from_utf8_lossy(&yaml_bytes);
                panic!(
                    "Our parser crashed on a case that serde_yaml accepted.\nFile: {}\nInput (lossy):\n{}",
                    file.display(), yaml_preview
                );
            }
            Ok(Ok(v)) => {
                eprintln!(
                    "summary: {} | ours: ok | serde_yaml: ok",
                    file.display()
                );
                v
            }
            Ok(Err(err)) => {
                eprintln!(
                    "summary: {} | ours: error | serde_yaml: ok",
                    file.display()
                );
                let yaml_preview = String::from_utf8_lossy(&yaml_bytes);
                panic!(
                    "Our parser failed to parse a case that serde_yaml accepted.\nFile: {}\nError: {err}\nInput (lossy):\n{}",
                    file.display(), yaml_preview
                );
            }
        };

        // Serialize our docs back to YAML using our serializer and compare by
        // re-parsing with serde_yaml into Values, then equality check.
        let bw_yaml = if bw_docs.len() == 1 {
            serde_saphyr::to_string(&bw_docs[0])?
        } else {
            serde_saphyr::to_string_multi(&bw_docs)?
        };

        let reparsed_by_serde = parse_all_with_serde_yaml_from_bytes(bw_yaml.as_bytes())?;
        assert_eq!(
            reparsed_by_serde, ser_docs,
            "Roundtrip via our serializer/Value changed semantics compared to serde_yaml.\nFile: {}\nOur emitted YAML:\n{}",
            file.display(), bw_yaml
        );

        // Additionally, serialize the serde_yaml Values using our serializer
        // and ensure our parser reads them back to an equivalent structure
        // (as judged again by serde_yaml).
        let ser_yaml_via_bw = if ser_docs.len() == 1 {
            serde_saphyr::to_string(&ser_docs[0])?
        } else {
            serde_saphyr::to_string_multi(&ser_docs)?
        };

        let reparsed_bw = parse_all_with_bw_from_bytes(ser_yaml_via_bw.as_bytes())?;
        let reparsed_bw_via_serde = parse_all_with_serde_yaml_from_bytes(
            serde_saphyr::to_string_multi(&reparsed_bw)?.as_bytes(),
        )?;
        assert_eq!(
            reparsed_bw_via_serde, ser_docs,
            "Serializing serde_yaml Values with our serializer, then parsing with our parser, should be semantics-preserving.\nFile: {}\nserde_yaml -> (our serializer) YAML:\n{}",
            file.display(), ser_yaml_via_bw
        );

        tested += 1;
    }

    eprintln!("yaml-test-suite differential: tested {} cases, skipped {}", tested, skipped);
    Ok(())
}
