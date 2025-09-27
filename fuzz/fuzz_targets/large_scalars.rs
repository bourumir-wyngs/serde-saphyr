#![no_main]

use libfuzzer_sys::fuzz_target;

// This fuzzer stresses large scalar handling, both plain and block scalars.
// We cap constructed sizes to avoid pathological memory usage.
fuzz_target!(|data: &[u8]| {
    if data.len() < 256 {
        return;
    }
    // Cap to 1 MiB generated content.
    let cap: usize = 1 << 20;

    // Repeat the fuzz input to build a long line.
    let mut plain = String::new();
    while plain.len() < cap {
        if plain.len() + data.len() > cap { break; }
        plain.push_str(&String::from_utf8_lossy(data));
    }

    // 1) Plain scalar
    let yaml_plain = format!("{plain}\n");

    // 2) Block literal scalar with folded lines
    let yaml_block = format!("|\n  {plain}\n  {plain}\n");

    for y in [&yaml_plain, &yaml_block] {
        let _s: Result<String, _> = serde_saphyr::from_str(y);
    }
});
