//! Tests for the indent_array option

use serde::Serialize;
use serde_saphyr::{SerializerOptions, to_fmt_writer_with_options};

#[derive(Serialize)]
struct Config {
    name: String,
    servers: Vec<Server>,
    settings: Settings,
}

#[derive(Serialize)]
struct Server {
    host: String,
    port: u16,
}

#[derive(Serialize)]
struct Settings {
    timeout: u32,
    retry: bool,
}

#[test]
fn test_default_indentation() {
    let config = Config {
        name: "test".to_string(),
        servers: vec![Server {
            host: "localhost".to_string(),
            port: 8080,
        }],
        settings: Settings {
            timeout: 30,
            retry: true,
        },
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(&mut buf, &config, SerializerOptions::default()).unwrap();

    // Both arrays and objects use 2-space indent by default
    assert!(buf.contains("servers:\n  - host: localhost\n    port: 8080\n"));
    assert!(buf.contains("settings:\n  timeout: 30\n  retry: true\n"));
}

#[test]
fn test_go_like_indentation() {
    let config = Config {
        name: "test".to_string(),
        servers: vec![Server {
            host: "localhost".to_string(),
            port: 8080,
        }],
        settings: Settings {
            timeout: 30,
            retry: true,
        },
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(
        &mut buf,
        &config,
        SerializerOptions {
            indent_step: 4,
            indent_array: Some(2),
            ..Default::default()
        },
    )
    .unwrap();

    // Arrays use 2-space indent, objects use 4-space indent
    assert!(buf.contains("servers:\n  - host: localhost\n    port: 8080\n"));
    assert!(buf.contains("settings:\n    timeout: 30\n    retry: true\n"));
}

#[test]
fn test_nested_arrays() {
    #[derive(Serialize)]
    struct Nested {
        matrix: Vec<Vec<i32>>,
    }

    let nested = Nested {
        matrix: vec![vec![1, 2], vec![3, 4]],
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(
        &mut buf,
        &nested,
        SerializerOptions {
            indent_step: 4,
            indent_array: Some(2),
            ..Default::default()
        },
    )
    .unwrap();

    // Nested arrays should use array indentation at each level
    assert!(buf.contains("matrix:\n  - - 1\n    - 2\n  - - 3\n    - 4\n"));
}

#[test]
fn test_custom_array_indentation() {
    #[derive(Serialize)]
    struct Simple {
        items: Vec<String>,
    }

    let simple = Simple {
        items: vec!["a".to_string(), "b".to_string()],
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(
        &mut buf,
        &simple,
        SerializerOptions {
            indent_step: 2,
            indent_array: Some(3),
            ..Default::default()
        },
    )
    .unwrap();

    // Arrays use 3-space indent, objects use 2-space indent
    assert!(buf.contains("items:\n   - a\n   - b\n"));
}

#[test]
fn test_array_of_objects_go_style() {
    #[derive(Serialize)]
    struct User {
        name: String,
        age: u32,
    }

    #[derive(Serialize)]
    struct TeamData {
        users: Vec<User>,
    }

    #[derive(Serialize)]
    struct Team {
        name: String,
        data: TeamData,
    }

    let data = Team {
        name: "Team 1".to_string(),
        data: TeamData {
            users: vec![
                User {
                    name: "Alice".to_string(),
                    age: 30,
                },
                User {
                    name: "Bob".to_string(),
                    age: 25,
                },
            ],
        },
    };

    let mut buf = String::new();
    to_fmt_writer_with_options(
        &mut buf,
        &data,
        SerializerOptions {
            indent_step: 4,
            indent_array: Some(0),
            ..Default::default()
        },
    )
    .unwrap();

    // Arrays use 2-space indent (the dash and array items)
    // Objects within arrays align their fields with the first field (2 spaces after dash)
    assert_eq!(
        buf,
        r#"name: Team 1
data:
    users:
    - name: Alice
      age: 30
    - name: Bob
      age: 25
"#
    );
}

#[test]
fn test_array_array_of_complex_objects() {
    #[derive(Serialize)]
    struct TriggerMetadata {
        metric_name: String,
        threshold: String,
    }

    #[derive(Serialize)]
    struct Trigger {
        metadata: TriggerMetadata,
        name: String,
        trigger_type: String,
    }

    #[derive(Serialize)]
    struct Spec {
        triggers: Vec<Trigger>,
        min_replica_count: u32,
        max_replica_count: u32,
    }

    #[derive(Serialize)]
    struct Manifest {
        name: String,
        spec: Spec,
    }

    let manifest = Manifest {
        name: "test".to_string(),
        spec: Spec {
            triggers: vec![Trigger {
                metadata: TriggerMetadata {
                    metric_name: "test".to_string(),
                    threshold: "100".to_string(),
                },
                name: "test".to_string(),
                trigger_type: "prometheus".to_string(),
            }],
            min_replica_count: 1,
            max_replica_count: 10,
        },
    };

    let mut buf = String::new();
    let options = serde_saphyr::SerializerOptions {
        indent_step: 4,
        indent_array: Some(4),
        ..Default::default()
    };
    to_fmt_writer_with_options(&mut buf, &manifest, options).unwrap();
    assert_eq!(
        buf,
        r#"name: test
spec:
    triggers:
        - metadata:
            metric_name: test
            threshold: '100'
          name: test
          trigger_type: prometheus
    min_replica_count: 1
    max_replica_count: 10
"#
    );
}
