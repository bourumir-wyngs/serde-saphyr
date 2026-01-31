// Collection of Grafana-derived integration tests.
//
// Note: Cargo only auto-discovers integration test crates at `tests/*.rs`.
// The individual test modules live under `tests/grafana/` and are included here.

#[path = "grafana/mod.rs"]
mod cases;
