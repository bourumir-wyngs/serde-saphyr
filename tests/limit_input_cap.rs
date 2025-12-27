use serde::Deserialize;
use serde_saphyr::{
    from_multiple_with_options, from_reader, from_reader_with_options, read, read_with_options, Budget
    , Error, Options,
};
use std::io::ErrorKind;
use serde_saphyr::budget::BudgetBreach;

fn unwrap_snippet(err: &Error) -> &Error {
    match err {
        Error::WithSnippet { error, .. } => error,
        other => other,
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct Simple {
    a1: i32,
}

fn big_valid_yaml(n: usize) -> String {
    let mut yaml = String::new();
    for i in 1..=n {
        yaml.push_str(&format!("a{}: {}\n", i, i));
    }
    yaml
}

#[test]
fn from_reader_respects_max_input_bytes_budget() {
    let yaml = big_valid_yaml(1024);
    let rdr = std::io::Cursor::new(yaml.as_bytes());

    let mut opts = Options::default();
    // Set a tiny max_input_bytes so the reader should trip it early
    opts.budget = Some(Budget {
        max_reader_input_bytes: Some(160),
        ..Budget::default()
    });

    let a: Result<Simple, Error> = from_reader_with_options(rdr.clone(), opts.clone());
    match a {
        Ok(_) => {
            assert!(false, "Should be able to limit max_input_bytes_budget");
        }
        Err(error) => match error {
            Error::IOError { cause } => {
                assert_eq!(cause.kind(), ErrorKind::FileTooLarge);
            }
            _ => assert!(false, "Unexpected error: {:?}", error),
        },
    }

    opts.budget = Some(Budget {
        max_reader_input_bytes: Some(16000),
        ..Budget::default()
    });
    let b: Result<Simple, Error> = from_reader_with_options(rdr.clone(), opts.clone());
    assert!(b.is_ok());

    let c: Result<Simple, Error> = from_reader(rdr.clone());
    assert!(c.is_ok());
}

#[test]
fn read_respects_max_input_bytes_budget() {
    let yaml = big_valid_yaml(1024);

    // Case 1: limit is hit (very small cap)
    let mut rdr1 = std::io::Cursor::new(yaml.as_bytes());
    let mut opts = Options::default();
    opts.budget = Some(Budget {
        max_reader_input_bytes: Some(160),
        ..Budget::default()
    });
    let mut iter1 = read_with_options::<_, Simple>(&mut rdr1, opts.clone());
    match iter1.next().unwrap() {
        Ok(_) => panic!("limit should have been hit and produced an error"),
        Err(Error::IOError { cause }) => assert_eq!(cause.kind(), ErrorKind::FileTooLarge),
        Err(other) => panic!("Unexpected error: {:?}", other),
    }
    // No further assertions about iterator termination; behavior after first error is not required here.

    // Case 2: limit is not hit (large cap)
    let mut rdr2 = std::io::Cursor::new(yaml.as_bytes());
    opts.budget = Some(Budget {
        max_reader_input_bytes: Some(16000),
        ..Budget::default()
    });
    let mut iter2 = read::<_, Simple>(&mut rdr2);
    let v = iter2
        .next()
        .expect("one item expected")
        .expect("should parse under budget");
    assert_eq!(v.a1, 1);
    assert!(iter2.next().is_none());

    // Case 3: limit set to None (no cap)
    let mut rdr3 = std::io::Cursor::new(yaml.as_bytes());
    opts.budget = Some(Budget {
        max_reader_input_bytes: None,
        ..Budget::default()
    });
    let mut iter3 = read_with_options::<_, Simple>(&mut rdr3, opts.clone());
    let v = iter3
        .next()
        .expect("one item expected")
        .expect("should parse with no cap");
    assert_eq!(v.a1, 1);
    assert!(iter3.next().is_none());
}

#[test]
fn read_limits_are_per_document() {
    let (opts, yaml) = yaml_and_options();
    let mut reader = std::io::Cursor::new(yaml.as_bytes());
    let mut iter = read_with_options::<_, Simple>(&mut reader, opts);

    let mut n = 0;
    while let Some(document) = iter.next() {
        let document = document.expect("Document expected");
        assert_eq!(document.a1, 1);
        n += 1;
    }
    assert_eq!(n, 5);

    // set now limit low enough for one document
    let mut opts = Options::default();
    opts.budget = Some(Budget {
        max_nodes: 30, // We have 1024 nodes per document
        ..Budget::default()
    });

    let deserialized: Result<Vec<Simple>, Error> = from_multiple_with_options(&yaml, opts);
    match deserialized {
        Ok(_) => panic!("limit should have been hit and produced an error"),
        Err(error) => match unwrap_snippet(&error) {
            Error::Budget { breach, .. } =>
                match breach {
                    BudgetBreach::Nodes { nodes } => { assert_eq!(nodes, &31)},
                    _ => assert!(false, "Unexpected kind of breach: {:?}", error),
                }
            _ => assert!(false, "Unexpected error: {:?}", error),
        },
    }
}

#[test]
// Same 5 documents and same budget limit
fn from_reader_limits_are_per_all_content() {
    let (opts, yaml) = yaml_and_options();
    let deserialized: Result<Vec<Simple>, Error> = from_multiple_with_options(&yaml, opts);
    match deserialized {
        Ok(_) => panic!("limit should have been hit and produced an error"),
        Err(error) => match unwrap_snippet(&error) {
            Error::Budget { breach, .. } =>
              match breach {
                  BudgetBreach::Nodes { nodes } => { assert_eq!(nodes, &3001)},
                  _ => assert!(false, "Unexpected kind of breach: {:?}", error),
              }
            _ => assert!(false, "Unexpected error: {:?}", error),
        },
    }
}

/// Create 5 documents but budget only for nodes for the first 3
fn yaml_and_options() -> (Options, String) {
    let mut opts = Options::default();
    let yaml = big_valid_yaml(1024);
    let yaml = format!("{yaml}\n---\n{yaml}\n---\n{yaml}\n---\n{yaml}\n---\n{yaml}\n");
    opts.budget = Some(Budget {
        max_nodes: 3000, // We have 1024 nodes per document
        ..Budget::default()
    });
    (opts, yaml)
}
