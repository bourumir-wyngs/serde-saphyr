use serde_saphyr::budget::{Budget, BudgetBreach, check_yaml_budget};

fn billion_laughs_yaml(levels: usize, fan_out: usize) -> String {
    assert!(levels > 0, "need at least one level");
    assert!(fan_out > 0, "fan_out must be positive");

    let mut yaml = String::new();
    yaml.push_str("l0: &L0 [\"LOL\", \"LOL\"]\n");
    for level in 1..=levels {
        yaml.push_str(&format!("l{level}: &L{level} ["));
        for idx in 0..fan_out {
            if idx > 0 {
                yaml.push_str(", ");
            }
            yaml.push_str(&format!("*L{}", level - 1));
        }
        yaml.push_str("]\n");
    }
    yaml.push_str(&format!("root: *L{levels}\n"));
    yaml
}

fn document_storm_yaml(count: usize) -> String {
    let mut yaml = String::new();
    for idx in 0..count {
        yaml.push_str("--- \"");
        yaml.push_str(&format!("doc{idx}"));
        yaml.push_str("\"\n");
    }
    yaml
}

#[test]
fn billion_laughs_is_rejected() {
    let yaml = billion_laughs_yaml(1, 128);
    let report = check_yaml_budget(&yaml, &Budget::default()).unwrap();
    assert!(
        matches!(
            report.breached,
            Some(BudgetBreach::AliasAnchorRatio { aliases, anchors })
                if aliases > anchors
        ),
        "expected alias/anchor ratio breach, got {:?}",
        report.breached
    );
}

#[test]
fn excessive_document_storm_is_rejected() {
    let limit = Budget::default().max_documents;
    let yaml = document_storm_yaml(limit + 1);
    let report = check_yaml_budget(&yaml, &Budget::default()).unwrap();
    assert!(
        matches!(report.breached, Some(BudgetBreach::Documents { .. })),
        "expected document limit breach, got {:?}",
        report.breached
    );
}

// tests/yaml_security_tests.rs

use serde::Deserialize;

/// Minimal user type for tests.
#[derive(Debug, Deserialize, PartialEq)]
struct User {
    username: String,
    hashed_password: String,
}

/// Strict server shape: we fail-closed on any unknown field to catch merge/injection attempts.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ServerStrict {
    host: String,
    port: u16,
    key: Vec<u8>,
    user: Vec<User>,
}

/// Loose server shape: allows unknowns so we can focus tests on alias recursion, etc.,
/// without tripping unknown-field errors unrelated to the attack.
#[derive(Debug, Deserialize, PartialEq)]
struct ServerLoose {
    host: String,
    port: u16,
    key: Vec<u8>,
    user: Vec<User>,
}

/// Helper: a known-good baseline YAML that should parse successfully.
fn good_yaml() -> &'static str {
    r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  Qm9iIGFuZCBBbGljZQ==
user:
  - username: alice
    hashed_password: "$argon2id$v=19$m=65536,t=3,p=4$C29tZXNhbHQ$eW91bGxORXZlckd1ZXNz"
  - username: bob
    hashed_password: "$argon2id$v=19$m=65536,t=3,p=4$YW5vdGhlcnNhbHQ$SW5zdGVhZEp1c3RFbmpveQo="
"#
}

#[test]
fn baseline_valid_config_parses() {
    let server: ServerStrict = serde_saphyr::from_str(good_yaml()).expect("valid YAML must parse");
    assert_eq!(server.host, "127.0.0.1");
    assert_eq!(server.port, 8080);
    assert_eq!(
        server.key,
        vec![
            0x42, 0x6f, 0x62, 0x20, 0x61, 0x6e, 0x64, 0x20, 0x41, 0x6c, 0x69, 0x63, 0x65
        ]
    );
    assert_eq!(server.user.len(), 2);
    assert_eq!(server.user[0].username, "alice");
    assert_eq!(server.user[1].username, "bob");
}

/// 1) Custom execution-capable tags (e.g., Python/Ruby) must be rejected.
/// Here we attempt to feed `!!python/object/apply:os.system` into `user`.
#[test]
fn rejects_custom_exec_tag_on_user() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  Qm9iIGFuZCBBbGljZQ==
user: !!python/object/apply:os.system
  - "echo pwned"
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(
        res.is_err(),
        "custom exec tags must not deserialize into typed fields"
    );
}

/// 2) Non-standard include tags should not be interpreted (avoid local file reads).
#[test]
fn rejects_include_tag_on_user() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  Qm9iIGFuZCBBbGljZQ==
user: !include "/etc/passwd"
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "!include must not be accepted by the parser");
}

/// 3) Self-referential alias (recursive anchor) should error instead of recursing indefinitely.
/// This is a safe, tiny "billion-laughs"-style check that does not allocate huge memory.
#[test]
fn rejects_self_referential_alias_in_sequence() {
    // Note: we use the "Loose" struct to keep focus on alias recursion rather than unknown fields.
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
# The 'user' sequence references itself: &u [*u]
user: &u [*u]
"#;
    let res: Result<ServerLoose, _> = serde_saphyr::from_str(yaml);
    assert!(
        res.is_err(),
        "self-referential alias must be detected and rejected"
    );
}

/// 4) Multiple documents in a single string should be rejected by single-document deserialization.
#[test]
fn rejects_multi_document_streams() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
user: []
---
host: 10.0.0.1
port: 9090
key: !!binary |
  QQ==
user: []
"#;
    let err = serde_saphyr::from_str::<ServerStrict>(yaml).expect_err("multi-doc must fail");
    match err {
        serde_saphyr::Error::Message { msg, .. } => {
            assert!(
                msg.contains("from_multiple"),
                "error should point users to from_multiple, got: {msg}"
            );
        }
        other => panic!("unexpected error variant: {:?}", other),
    }
}

/// 5) Invalid base64 in `!!binary` must be rejected, preventing garbage in byte vectors.
#[test]
fn rejects_invalid_base64_for_binary_key() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
# Intentionally invalid base64 payload (contains '@')
key: !!binary |
  QEA=@
user: []
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "invalid base64 must cause an error");
}

/// 6) Smuggling non-UTF-8 into a `String` using `!!binary` must fail.
#[test]
fn rejects_non_utf8_username_via_binary_tag() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
user:
  - username: !!binary |
      AQID   # [0x01, 0x02, 0x03] -> not valid UTF-8 as a full string
    hashed_password: "x"
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(
        res.is_err(),
        "non-UTF-8 scalar must not deserialize into String"
    );
}

#[test]
fn allows_utf8_username_via_binary_tag() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
user:
  - username: !!binary |
      aGk=
    hashed_password: "pw"
"#;
    let parsed: ServerStrict = serde_saphyr::from_str(yaml).expect("valid UTF-8 binary allowed");
    assert_eq!(parsed.user[0].username, "hi");
}

/// 7) Explicit non-string scalar coerced into a string field (e.g., !!bool) must fail.
#[test]
fn rejects_bool_tag_in_string_field() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
user:
  - username: !!bool true
    hashed_password: "pw"
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(
        res.is_err(),
        "typed booleans must not deserialize into String"
    );
}

#[test]
fn rejects_int_tag_in_string_field() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
user:
  - username: !!int 42
    hashed_password: "pw"
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(
        res.is_err(),
        "typed integers must not deserialize into String"
    );
}

/// 8) Timestamps or other mismatched typed scalars must not deserialize into numeric fields.
#[test]
fn rejects_timestamp_for_numeric_port() {
    let yaml = r#"
---
host: 127.0.0.1
port: !!timestamp 2020-01-01T00:00:00Z
key: !!binary |
  QQ==
user: []
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "timestamp must not deserialize into u16");
}

/// 9) Numeric overflow (e.g., port out of u16 range) must error rather than wrap.
#[test]
fn rejects_overflowing_port() {
    let yaml = r#"
---
host: 127.0.0.1
port: 70000
key: !!binary |
  QQ==
user: []
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "overflowing u16 must be rejected");
}

/// 10) Merge-key injection (`<<`) must not silently introduce unknown fields in strict mode.
/// This test passes whether the loader expands YAML merges or treats `<<` as a literal key:
/// - If merges are supported: `extra` becomes a real top-level key -> denied by `deny_unknown_fields`.
/// - If not: `<<` itself is an unknown key -> also denied by `deny_unknown_fields`.
#[test]
fn rejects_merge_key_injection_in_strict_shape() {
    let yaml = r#"
---
# Attempt to merge in extra fields and wrong-typed values
<<: &inject { extra: "bad", port: "not-a-number" }
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
user: []
"#;
    let res: Result<ServerStrict, _> = serde_saphyr::from_str(yaml);
    assert!(
        res.is_err(),
        "merge-key injection or unknown fields must be rejected in strict shape"
    );
}

/// 11) Control: safe anchor reuse for repeated objects should still work and remain small.
/// This demonstrates legitimate anchors are fine (not an attack), and keeps expansion bounded.
#[test]
fn alias_reuse_for_repeated_users_is_ok() {
    let yaml = r#"
---
host: 127.0.0.1
port: 8080
key: !!binary |
  QQ==
# Define one user mapping, then reuse it twice. This is safe and small.
u: &u_elt
  username: alice
  hashed_password: "hpw"
user: [*u_elt, *u_elt]
"#;
    // Using the loose shape to ignore the helper top-level "u" (not part of the schema under test).
    let parsed: ServerLoose = serde_saphyr::from_str(yaml).expect("safe alias reuse must parse");
    assert_eq!(parsed.user.len(), 2);
    assert_eq!(parsed.user[0], parsed.user[1]);
    assert_eq!(parsed.user[0].username, "alice");
}
