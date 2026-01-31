// `trybuild` runs by spawning a host `cargo` process to compile UI test crates.
// This is not supported on WASI runtimes, so we disable these tests on WASI.
// Miri cannot run tests that require spawning processes, so we disable these tests under Miri.
#![cfg(not(target_os = "wasi"))]
#![cfg(not(miri))]

#[test]
fn serializer_options_indent_step_range_is_enforced_for_literals() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/indent_step_ok.rs");
    t.compile_fail("tests/ui/indent_step_zero.rs");
    t.compile_fail("tests/ui/indent_step_too_large.rs");
    t.compile_fail("tests/ui/indent_step_huge.rs");
}
