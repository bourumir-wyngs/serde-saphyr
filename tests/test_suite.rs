// Collection of yaml-test-suite derived integration tests.
//
// Note: Cargo only auto-discovers integration test crates at `tests/*.rs`.
// The individual test modules live under `tests/test_suite/` and are included here.

#[path = "test_suite/mod.rs"]
mod cases;
