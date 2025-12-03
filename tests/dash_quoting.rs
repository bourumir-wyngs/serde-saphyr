//! Tests for YAML quoting behavior matching Go's yaml.v3
//!
//! These tests verify that serde-saphyr produces YAML output compatible with
//! Go's gopkg.in/yaml.v3, particularly around quoting of strings starting with
//! dash `-` and strings containing commas in block style.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Container {
    args: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct MapContainer {
    data: HashMap<String, String>,
}

// =============================================================================
// Dash-prefixed strings (the main fix for Go yaml.v3 compatibility)
// =============================================================================

#[test]
fn test_dash_prefix_not_quoted() {
    let c = Container {
        args: vec![
            "-server.port=80".to_string(),
            "-config.file=/etc/config.yaml".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // These should NOT be quoted (matching Go's yaml.v3 behavior)
    assert!(
        yaml.contains("- -server.port=80"),
        "Expected unquoted -server.port=80, got:\n{}",
        yaml
    );
    assert!(
        yaml.contains("- -config.file=/etc/config.yaml"),
        "Expected unquoted -config.file, got:\n{}",
        yaml
    );
    // Should NOT contain quoted versions
    assert!(
        !yaml.contains("\"-server.port=80\""),
        "Should not be quoted:\n{}",
        yaml
    );
}

#[test]
fn test_dash_alone_is_quoted() {
    let c = Container {
        args: vec!["-".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Single dash should be quoted (could be confused with empty sequence indicator)
    assert!(
        yaml.contains("\"-\"") || yaml.contains("'-'"),
        "Single dash should be quoted:\n{}",
        yaml
    );
}

#[test]
fn test_dash_space_prefix_is_quoted() {
    let c = Container {
        args: vec!["- starts with dash space".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Dash-space prefix should be quoted (would be sequence indicator)
    assert!(
        yaml.contains("\"- starts with dash space\""),
        "Dash-space prefix should be quoted:\n{}",
        yaml
    );
}

#[test]
fn test_dash_followed_by_various_chars() {
    // Test various characters after dash - all should be unquoted
    let c = Container {
        args: vec![
            "-a".to_string(),
            "-_underscore".to_string(),
            "--double-dash".to_string(),
            "-foo=bar".to_string(),
            "-foo.bar.baz".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    assert!(yaml.contains("- -a"), "Expected unquoted -a");
    assert!(
        yaml.contains("- -_underscore"),
        "Expected unquoted -_underscore"
    );
    assert!(
        yaml.contains("- --double-dash"),
        "Expected unquoted --double-dash"
    );
    assert!(yaml.contains("- -foo=bar"), "Expected unquoted -foo=bar");
    assert!(
        yaml.contains("- -foo.bar.baz"),
        "Expected unquoted -foo.bar.baz"
    );
}

#[test]
fn test_dash_number_is_quoted() {
    // -1, -2, etc. look like negative numbers, so they should be quoted to preserve string type
    let c = Container {
        args: vec!["-1".to_string(), "-123".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // These should be quoted because they look like negative integers
    assert!(
        yaml.contains("\"-1\"") || yaml.contains("'-1'"),
        "-1 should be quoted (looks like negative number):\n{}",
        yaml
    );
}

#[test]
fn test_dash_tab_is_quoted() {
    let c = Container {
        args: vec!["-\ttab after dash".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Dash followed by tab should be quoted (whitespace after dash)
    assert!(
        yaml.contains('"') || yaml.contains('\''),
        "Dash-tab should be quoted:\n{}",
        yaml
    );
}

// =============================================================================
// Question mark behavior (similar to dash)
// =============================================================================

#[test]
fn test_question_mark_prefix_not_quoted() {
    let c = Container {
        args: vec!["?query=value".to_string(), "?foo".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    assert!(
        yaml.contains("- ?query=value"),
        "Expected unquoted ?query=value, got:\n{}",
        yaml
    );
    assert!(
        yaml.contains("- ?foo"),
        "Expected unquoted ?foo, got:\n{}",
        yaml
    );
}

#[test]
fn test_question_mark_alone_is_quoted() {
    let c = Container {
        args: vec!["?".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    assert!(
        yaml.contains("\"?\"") || yaml.contains("'?'"),
        "Single ? should be quoted:\n{}",
        yaml
    );
}

#[test]
fn test_question_mark_space_is_quoted() {
    let c = Container {
        args: vec!["? mapping key indicator".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    assert!(yaml.contains('"'), "?-space should be quoted:\n{}", yaml);
}

// =============================================================================
// Commas in block style (should be allowed)
// =============================================================================

#[test]
fn test_commas_in_values_not_quoted_block_style() {
    let c = Container {
        args: vec![
            "a,b,c".to_string(),
            "service.name,service.namespace".to_string(),
            "-flag=value1,value2,value3".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Commas within values should NOT be quoted in block style
    assert!(
        yaml.contains("- a,b,c"),
        "Expected unquoted a,b,c, got:\n{}",
        yaml
    );
    assert!(
        yaml.contains("- service.name,service.namespace"),
        "Expected unquoted comma-separated names"
    );
    assert!(
        yaml.contains("- -flag=value1,value2,value3"),
        "Expected unquoted flag with commas"
    );
}

#[test]
fn test_standalone_comma_is_quoted() {
    let c = Container {
        args: vec![",".to_string()],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Standalone comma should be quoted (structural character)
    assert!(
        yaml.contains("\",\"") || yaml.contains("','"),
        "Standalone comma should be quoted:\n{}",
        yaml
    );
}

#[test]
fn test_standalone_brackets_are_quoted() {
    let c = Container {
        args: vec![
            "[".to_string(),
            "]".to_string(),
            "{".to_string(),
            "}".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Standalone brackets should be quoted
    assert!(
        yaml.contains("\"[\"") || yaml.contains("'['"),
        "[ should be quoted:\n{}",
        yaml
    );
    assert!(
        yaml.contains("\"]\"") || yaml.contains("']'"),
        "] should be quoted:\n{}",
        yaml
    );
    assert!(
        yaml.contains("\"{\"") || yaml.contains("'{'"),
        "{{ should be quoted:\n{}",
        yaml
    );
    assert!(
        yaml.contains("\"}\"") || yaml.contains("'}'"),
        "}} should be quoted:\n{}",
        yaml
    );
}

#[test]
fn test_brackets_within_values_not_quoted() {
    let c = Container {
        args: vec![
            "array[0]".to_string(),
            "map{key}".to_string(),
            "[brackets]inside".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();
    // Note: strings STARTING with [ or { will be quoted, but brackets within are ok
    // The first char check catches [brackets]inside
    assert!(
        yaml.contains("- array[0]"),
        "Expected unquoted array[0], got:\n{}",
        yaml
    );
    assert!(
        yaml.contains("- map{key}"),
        "Expected unquoted map{{key}}, got:\n{}",
        yaml
    );
}

// =============================================================================
// Round-trip tests
// =============================================================================

#[test]
fn test_roundtrip_dash_prefixed() {
    let original = Container {
        args: vec![
            "-server.port=80".to_string(),
            "-config.file=/etc/config.yaml".to_string(),
            "-verbose".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&original).unwrap();
    let parsed: Container = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_roundtrip_commas() {
    let original = Container {
        args: vec![
            "a,b,c".to_string(),
            "one,two,three".to_string(),
            "-flag=x,y,z".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&original).unwrap();
    let parsed: Container = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_roundtrip_special_chars() {
    let original = Container {
        args: vec![
            "-".to_string(),
            "?".to_string(),
            ",".to_string(),
            "- dash space".to_string(),
            "? question space".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&original).unwrap();
    let parsed: Container = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(original, parsed);
}

// =============================================================================
// Flow style tests (commas/brackets MUST be quoted in flow style)
// =============================================================================

#[test]
fn test_flow_style_quotes_structural_chars() {
    use serde_saphyr::FlowSeq;

    let c = FlowSeq(vec!["a,b".to_string(), "x".to_string()]);
    let yaml = serde_saphyr::to_string(&c).unwrap();

    // In flow style, strings with commas should be quoted
    // The output should be like: ["a,b", x] or similar
    assert!(
        yaml.contains("\"a,b\"") || yaml.contains("'a,b'"),
        "Comma in flow style should be quoted:\n{}",
        yaml
    );
}

// =============================================================================
// Map values (not just sequence items)
// =============================================================================

#[test]
fn test_map_values_dash_prefix() {
    let mut data = HashMap::new();
    data.insert("flag".to_string(), "-verbose".to_string());
    data.insert("config".to_string(), "-c=/etc/config.yaml".to_string());

    let c = MapContainer { data };
    let yaml = serde_saphyr::to_string(&c).unwrap();

    // Map values with dash prefix should also be unquoted
    assert!(
        yaml.contains(": -verbose") || yaml.contains(": -c="),
        "Map values with dash prefix should be unquoted:\n{}",
        yaml
    );
}

#[test]
fn test_map_values_with_commas() {
    let mut data = HashMap::new();
    data.insert("list".to_string(), "a,b,c".to_string());

    let c = MapContainer { data };
    let yaml = serde_saphyr::to_string(&c).unwrap();

    // Map values with commas should be unquoted in block style
    assert!(
        yaml.contains(": a,b,c"),
        "Map values with commas should be unquoted:\n{}",
        yaml
    );
}

// =============================================================================
// Real-world Kubernetes-like arguments
// =============================================================================

#[test]
fn test_kubernetes_style_args() {
    let c = Container {
        args: vec![
            "-server.metrics-port=80".to_string(),
            "-config.file=/etc/mimir/config.yaml".to_string(),
            "-distributor.ha-tracker.enable=true".to_string(),
            "-blocks-storage.s3.bucket-name=my-bucket".to_string(),
            "-ingester.ring.replication-factor=3".to_string(),
            "-querier.query-ingesters-within=13h".to_string(),
            "-runtime-config.file=/etc/mimir/runtime.yaml".to_string(),
            "-memberlist.join=dns+memberlist.default.svc.cluster.local:7946".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();

    // All these Kubernetes-style args should be unquoted
    for arg in &c.args {
        assert!(
            yaml.contains(&format!("- {}", arg)),
            "Expected unquoted {}, got:\n{}",
            arg,
            yaml
        );
    }
}

#[test]
fn test_kubernetes_args_with_commas() {
    // This is the exact pattern that was causing issues
    let c = Container {
        args: vec![
            "-distributor.otelpromoter.tiered-resource-attribute-promotion.fallback-resource-attribute-promotion-list=service.instance.id,service.name,service.namespace".to_string(),
            "-ingest-storage.kafka.client-id=warpstream_az=eu-south-2a,ws_pas=equal_spread,ws_pt=proxy-produce".to_string(),
        ],
    };

    let yaml = serde_saphyr::to_string(&c).unwrap();

    // These args with commas should NOT be quoted
    assert!(
        !yaml.contains('"'),
        "No quotes expected for args with commas in block style:\n{}",
        yaml
    );

    // Verify round-trip works
    let parsed: Container = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(c, parsed);
}
