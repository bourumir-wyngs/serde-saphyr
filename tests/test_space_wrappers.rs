//! Tests for SpaceAfter wrapper.

use serde::{Deserialize, Serialize};
use serde_saphyr::{Commented, FlowSeq, SpaceAfter, from_str, to_string};

#[test]
fn space_after_adds_blank_line() {
    #[derive(Serialize)]
    struct Config {
        first: SpaceAfter<i32>,
        second: i32,
    }

    let cfg = Config {
        first: SpaceAfter(1),
        second: 2,
    };
    let yaml = to_string(&cfg).unwrap();

    // Should have a blank line after "first: 1"
    assert!(
        yaml.contains("first: 1\n\n"),
        "Expected blank line after 'first: 1', got:\n{yaml}"
    );
}

#[test]
fn space_after_with_commented() {
    #[derive(Serialize)]
    struct Config {
        value: SpaceAfter<Commented<i32>>,
        next: i32,
    }

    let cfg = Config {
        value: SpaceAfter(Commented(42, "important value".into())),
        next: 1,
    };
    let yaml = to_string(&cfg).unwrap();

    // Should have comment and blank line after value
    assert!(
        yaml.contains("# important value"),
        "Expected comment, got:\n{yaml}"
    );
    assert!(
        yaml.contains("\n\nnext:"),
        "Expected blank line before 'next:', got:\n{yaml}"
    );
}

#[test]
fn space_after_with_flow_seq() {
    #[derive(Serialize)]
    struct Config {
        items: SpaceAfter<FlowSeq<Vec<i32>>>,
        next: i32,
    }

    let cfg = Config {
        items: SpaceAfter(FlowSeq(vec![1, 2, 3])),
        next: 42,
    };
    let yaml = to_string(&cfg).unwrap();

    // Should have flow style sequence with blank line after
    assert!(
        yaml.contains("[1, 2, 3]"),
        "Expected flow sequence, got:\n{yaml}"
    );
    assert!(
        yaml.contains("]\n\n"),
        "Expected blank line after flow sequence, got:\n{yaml}"
    );
}

#[test]
fn space_after_with_sequence() {
    #[derive(Serialize)]
    struct Config {
        items: SpaceAfter<Vec<i32>>,
        next: i32,
    }

    let cfg = Config {
        items: SpaceAfter(vec![1, 2, 3]),
        next: 42,
    };
    let yaml = to_string(&cfg).unwrap();

    // Should have blank line after the sequence
    assert!(
        yaml.contains("- 3\n\n"),
        "Expected blank line after sequence, got:\n{yaml}"
    );
}

#[test]
fn multiple_space_after_sections() {
    #[derive(Serialize)]
    struct Config {
        section1: SpaceAfter<String>,
        section2: SpaceAfter<String>,
        section3: String,
    }

    let cfg = Config {
        section1: SpaceAfter("first".into()),
        section2: SpaceAfter("second".into()),
        section3: "third".into(),
    };
    let yaml = to_string(&cfg).unwrap();

    // Should have blank lines after section1 and section2
    assert!(
        yaml.contains("first\n\n"),
        "Expected blank line after section1, got:\n{yaml}"
    );
    assert!(
        yaml.contains("second\n\n"),
        "Expected blank line after section2, got:\n{yaml}"
    );
}

#[test]
fn space_after_suppressed_in_flow() {
    #[derive(Serialize)]
    struct Config {
        items: FlowSeq<Vec<SpaceAfter<i32>>>,
    }

    let cfg = Config {
        items: FlowSeq(vec![SpaceAfter(1), SpaceAfter(2), SpaceAfter(3)]),
    };
    let yaml = to_string(&cfg).unwrap();

    // Inside flow context, SpaceAfter should be suppressed
    assert!(
        yaml.contains("[1, 2, 3]"),
        "Expected flow sequence without extra spaces, got:\n{yaml}"
    );
}

#[test]
fn space_after_deserialize_transparent() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Config {
        value: SpaceAfter<i32>,
    }

    let yaml = "value: 42\n";
    let cfg: Config = from_str(yaml).unwrap();

    assert_eq!(cfg.value.0, 42);
}

#[test]
fn space_after_nested() {
    #[derive(Serialize)]
    struct Inner {
        a: i32,
    }

    #[derive(Serialize)]
    struct Config {
        inner: SpaceAfter<Inner>,
        next: i32,
    }

    let cfg = Config {
        inner: SpaceAfter(Inner { a: 1 }),
        next: 2,
    };
    let yaml = to_string(&cfg).unwrap();

    // Should have blank line after the nested struct
    assert!(
        yaml.contains("a: 1\n\n"),
        "Expected blank line after inner struct, got:\n{yaml}"
    );
}

#[test]
fn space_after_with_string() {
    #[derive(Serialize)]
    struct Config {
        message: SpaceAfter<String>,
        count: i32,
    }

    let cfg = Config {
        message: SpaceAfter("hello world".into()),
        count: 42,
    };
    let yaml = to_string(&cfg).unwrap();

    assert!(
        yaml.contains("hello world\n\n"),
        "Expected blank line after string, got:\n{yaml}"
    );
}
