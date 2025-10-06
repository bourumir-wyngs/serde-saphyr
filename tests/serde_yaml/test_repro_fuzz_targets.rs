use std::fs;
use std::path::{Path, PathBuf};

// Shared: recursively collect all files inside a base directory. If the base
// dir does not exist, return empty and let the test skip.
// The files are numerous and bulky, so we do not commit to git so far
fn collect_files_recursive(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !base.exists() {
        return Ok(out);
    }
    fn walk(dir: &Path, acc: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, acc)?;
            } else if path.is_file() {
                acc.push(path);
            }
        }
        Ok(())
    }
    walk(base, &mut out)?;
    Ok(out)
}

// ===== aliases/merges (fuzzer folder alias_merges) =====
fn run_aliases_merges_on(data: &[u8]) -> bool {
    if data.len() > 16 * 1024 {
        // Skip overly large seed inputs similarly to the fuzzer harness.
        return true;
    }

    let s = String::from_utf8_lossy(data);

    // 1) Anchors and aliases only.
    let yaml_alias = format!("a: &A {s}\nb: *A\nseq: &S [1, 2, 3]\nseq_alias: *S\n");

    // 2) Merge keys scenario.
    let yaml_merge = format!(
        "base1: &B1 {{k: 1, v: {s}}}\nbase2: &B2 {{k: 2, w: {s}}}\nmerged: {{<<: [*B1, *B2], extra: 3}}\n"
    );

    let budget = Budget::default();
    if exceeds_yaml_budget(&yaml_alias, &budget).is_err() {
        return true;
    }
    if exceeds_yaml_budget(&yaml_merge, &budget).is_err() {
        return true;
    }
    false
}

// ===== duplicate_keys =====
fn run_duplicate_keys_on(data: &[u8]) -> bool {
    if data.len() > 16 * 1024 {
        return true;
    }
    let s = String::from_utf8_lossy(data);

    let yaml_top = format!("a: 1\na: 2\nkey: {s}\nkey: {s}\n");
    let yaml_nested = format!("outer:\n  inner: {{x: 1, x: 2}}\n  arr: [{{k: {s}}}, {{k: {s}}}]\n");
    let budget = Budget::default();
    if exceeds_yaml_budget(&yaml_top, &budget).is_err() {
        return true;
    }
    if exceeds_yaml_budget(&yaml_nested, &budget).is_err() {
        return true;
    }
    return false;

}

// ===== flow_collections =====
fn run_flow_collections_on(data: &[u8]) -> bool {
    if data.len() > 16 * 1024 {
        return true;
    }

    let s = String::from_utf8_lossy(data);
    let yaml_seq = format!("[{s}]");
    let yaml_map = format!("{{{s}}}");
    let yaml_doc = format!("root: {{{s}}}\narray: [{s}]\n");

    let budget = Budget::default();
    if exceeds_yaml_budget(&yaml_seq, &budget).is_err() {
        return true;
    }
    if exceeds_yaml_budget(&yaml_map, &budget).is_err() {
        return true;
    }
    if exceeds_yaml_budget(&yaml_doc, &budget).is_err() {
        return true;
    }
    false
}

// ===== large_scalars =====
fn run_large_scalars_on(data: &[u8]) -> bool {
    if data.len() < 256 {
        // Fuzzer ignores inputs < 256 bytes; treat as non-applicable.
        return true;
    }
    let cap: usize = 1 << 20; // 1 MiB cap like in the fuzzer

    let mut plain = String::new();
    while plain.len() < cap {
        if plain.len() + data.len() > cap {
            break;
        }
        plain.push_str(&String::from_utf8_lossy(data));
    }

    let yaml_plain = format!("{plain}\n");
    let yaml_block = format!("|\n  {plain}\n  {plain}\n");
    let budget = Budget::default();

    if exceeds_yaml_budget(&yaml_plain, &budget).is_err() {
        return true;
    }

    if exceeds_yaml_budget(&yaml_block, &budget).is_err() {
        return true;
    }
    return false;
}

// Test 1: aliases/merges repro
#[test]
fn repro_alias_merges_crashes() {
    let base = Path::new("tests/fuzz_crashes/alias_merges");
    let files = match collect_files_recursive(base) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };
    if files.is_empty() {
        return;
    }
    for f in files {
        let data = match fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("read {} failed: {e}", f.display());
                continue;
            }
        };
        let ok = run_aliases_merges_on(&data);
        assert!(
            ok,
            "aliases/merges input from {} was not detected as pathological",
            f.display()
        );
    }
}

// Test 2: duplicate_keys repro
#[test]
fn repro_duplicate_keys_crashes() {
    let base = Path::new("tests/fuzz_crashes/duplicate_keys");
    let files = match collect_files_recursive(base) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };
    if files.is_empty() {
        return;
    }
    for f in files {
        let data = match fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("read {} failed: {e}", f.display());
                continue;
            }
        };
        let ok = run_duplicate_keys_on(&data);
        assert!(
            ok,
            "duplicate_keys input from {} was not detected as pathological",
            f.display()
        );
    }
}

// Test 3: flow_collections repro
#[test]
fn repro_flow_collections_crashes() {
    let base = Path::new("tests/fuzz_crashes/flow_collections");
    let files = match collect_files_recursive(base) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };
    if files.is_empty() {
        return;
    }
    for f in files {
        let data = match fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("read {} failed: {e}", f.display());
                continue;
            }
        };
        let ok = run_flow_collections_on(&data);
        assert!(
            ok,
            "flow_collections input from {} was not detected as pathological",
            f.display()
        );
    }
}

// Test 4: large_scalars repro
#[test]
fn repro_large_scalars_crashes() {
    let base = Path::new("tests/fuzz_crashes/large_scalars");
    let files = match collect_files_recursive(base) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };
    if files.is_empty() {
        return;
    }
    for f in files {
        let data = match fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("read {} failed: {e}", f.display());
                continue;
            }
        };
        let ok = run_large_scalars_on(&data);
        assert!(
            ok,
            "large_scalars input from {} was not detected as pathological",
            f.display()
        );
    }
}

use serde_saphyr::budget::{exceeds_yaml_budget, Budget};
use std::str;

/// Convert possibly non-UTF-8 bytes to a `String`, preserving valid UTF-8
/// and marking invalid byte sequences as `/hex:..../`.
///
/// # Parameters
/// - `bytes`: The raw byte slice that may contain invalid UTF-8.
///
/// # Returns
/// - `String`: A string where valid UTF-8 is kept intact and each invalid
///   byte sequence is replaced by a marker like `/hex:FF/` or `/hex:C3-28/`.
pub fn bytes_to_marked_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        match str::from_utf8(&bytes[i..]) {
            Ok(s) => {
                // The remainder is valid UTF-8; append and finish.
                out.push_str(s);
                break;
            }
            Err(e) => {
                // Append the valid UTF-8 prefix (if any).
                let good_len = e.valid_up_to();
                if good_len > 0 {
                    // This slice is guaranteed valid UTF-8.
                    out.push_str(unsafe { str::from_utf8_unchecked(&bytes[i..i + good_len]) });
                    i += good_len;
                }

                // Determine how many bytes are in the invalid sequence.
                // If `None`, treat as 1 trailing problematic byte.
                let bad_len = e
                    .error_len()
                    .unwrap_or(1)
                    .min(bytes.len().saturating_sub(i));
                if bad_len == 0 {
                    break;
                }

                // Emit the marker for the invalid bytes as lowercase hex pairs.
                out.push_str("/hex:");
                for (k, b) in bytes[i..i + bad_len].iter().enumerate() {
                    if k > 0 {
                        out.push('-');
                    }
                    // always two hex digits
                    out.push(nibble_to_hex(b >> 4));
                    out.push(nibble_to_hex(b & 0x0F));
                }
                out.push('/');

                i += bad_len;
            }
        }
    }

    out
}

/// Map a 4-bit nibble to a lowercase hex char.
fn nibble_to_hex(n: u8) -> char {
    let n = (n & 0x0F) as u32;
    char::from_u32(if n < 10 {
        b'0' as u32 + n
    } else {
        b'a' as u32 + (n - 10)
    })
    .unwrap()
}
