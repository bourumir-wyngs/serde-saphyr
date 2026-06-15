#![cfg(all(feature = "serialize", feature = "deserialize"))]
// `trybuild` runs by spawning a host `cargo` process to compile UI test crates.
#![cfg(not(target_os = "wasi"))]
#![cfg(not(miri))]

#[test]
fn quoted_only_supports_string_like_types() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/quoted_string_ok.rs");
    t.compile_fail("tests/ui/quoted_non_string.rs");
}
