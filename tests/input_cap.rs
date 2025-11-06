use serde::Deserialize;
use serde_saphyr::{from_reader, from_reader_with_options, from_str_with_options, Budget, Error, Options};

#[derive(Debug, Deserialize, PartialEq)]
struct Simple { a1: i32 }

fn big_valid_yaml(n: usize) -> String {
    let mut yaml = String::new();
    for i in 1..=n {
        yaml.push_str(&format!("a{}: {}\n", i, i));
    }
    yaml
}

#[test]
fn reader_respects_max_input_bytes_budget() {
    let yaml = big_valid_yaml(1024);
    let rdr = std::io::Cursor::new(yaml.as_bytes());

    let mut opts = Options::default();
    // Set a tiny max_input_bytes so the reader should trip it early
    opts.budget = Some(Budget { max_input_bytes: 160, ..Budget::default() });

    let a: Result<Simple, Error> = from_reader_with_options(rdr.clone(), opts.clone());
    assert!(a.is_err());

    opts.budget = Some(Budget { max_input_bytes: 16000, ..Budget::default() });
    let b: Result<Simple, Error> = from_reader_with_options(rdr.clone(), opts.clone());
    assert!(b.is_ok());

    let c: Result<Simple, Error> = from_reader(rdr.clone());
    assert!(c.is_ok());
}

#[test]
fn string_respects_max_input_bytes_budget() {
    let yaml = big_valid_yaml(1024);

    let mut opts = Options::default();
    // Set a tiny max_input_bytes to trigger pre-check on string length
    opts.budget = Some(Budget { max_input_bytes: 8, ..Budget::default() });

    let err = from_str_with_options::<Simple>(&yaml, opts).expect_err("expected budget error");
    let msg = err.to_string();
    assert!(msg.contains("YAML budget breached"), "unexpected msg: {}", msg);
    assert!(msg.contains("InputBytes"), "expected InputBytes breach, got: {}", msg);
}
