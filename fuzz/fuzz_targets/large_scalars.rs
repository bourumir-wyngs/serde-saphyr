#![no_main]

use libfuzzer_sys::fuzz_target;

// This fuzzer stresses large scalar handling, both plain and block scalars.
// We cap constructed sizes to avoid pathological memory usage.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 64 * 1024 {
        return;
    }
    // Small inputs must reach the parser too, so an empty corpus can grow.
    // Vary the generated size from 256 bytes to 1 MiB and decode UTF-8 only once.
    let cap: usize = 1 << (8 + data[0] % 13);
    let fragment = String::from_utf8_lossy(&data[..data.len().min(4096)]);
    let plain = fragment.repeat((cap / fragment.len()).max(1));

    // 1) Plain scalar
    let yaml_plain = format!("{plain}\n");

    // 2) Block literal scalar with folded lines
    let yaml_block = format!("|\n  {plain}\n  {plain}\n");

    for y in [&yaml_plain, &yaml_block] {
        let _s: Result<String, _> = serde_saphyr::from_str(y);
    }
});
