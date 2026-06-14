#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Serialize;
use serde_saphyr::to_string_with_options;

#[test]
fn compact_list_indent_default() {
    #[derive(Serialize)]
    struct Env {
        name: &'static str,
        value: &'static str,
    }

    #[derive(Serialize)]
    struct Container {
        env: Vec<Env>,
    }

    #[derive(Serialize)]
    struct Spec {
        containers: Vec<Container>,
    }

    let spec = Spec {
        containers: vec![Container {
            env: vec![
                Env {
                    name: "METHOD",
                    value: "WATCH",
                },
                Env {
                    name: "LABEL",
                    value: "grafana_dashboard",
                },
            ],
        }],
    };

    let opts = serde_saphyr::ser_options! {
        compact_list_indent: false,
    };
    let yaml = to_string_with_options(&spec, opts).unwrap();
    let lines: Vec<&str> = yaml.lines().collect();
    assert_eq!(lines[0], "containers:");
    assert_eq!(lines[1], "  - env:");
    assert_eq!(lines[2], "      - name: METHOD");
    assert_eq!(lines[3], "        value: WATCH");
}

#[test]
fn compact_list_indent_enabled() {
    #[derive(Serialize)]
    struct Env {
        name: &'static str,
        value: &'static str,
    }

    #[derive(Serialize)]
    struct Container {
        env: Vec<Env>,
    }

    #[derive(Serialize)]
    struct Spec {
        containers: Vec<Container>,
    }

    let spec = Spec {
        containers: vec![Container {
            env: vec![
                Env {
                    name: "METHOD",
                    value: "WATCH",
                },
                Env {
                    name: "LABEL",
                    value: "grafana_dashboard",
                },
            ],
        }],
    };

    let opts = serde_saphyr::ser_options! {
        compact_list_indent: true,
    };
    let yaml = to_string_with_options(&spec, opts).unwrap();
    let lines: Vec<&str> = yaml.lines().collect();
    assert_eq!(lines[0], "containers:");
    assert_eq!(lines[1], "- env:");
    assert_eq!(lines[2], "  - name: METHOD");
    assert_eq!(lines[3], "    value: WATCH");
}

#[test]
fn tuple_struct_serializes_like_a_vec() {
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct Tup(u8, u8, u8, u8);

    let tup = Tup(9, 19, 11, 21);
    let vec = vec![9u8, 19, 11, 21];

    assert_eq!(
        serde_saphyr::to_string(&tup).unwrap(),
        serde_saphyr::to_string(&vec).unwrap(),
    );

    let as_tup = BTreeMap::from([("hello", BTreeMap::from([("world", &tup)]))]);
    let as_vec = BTreeMap::from([("hello", BTreeMap::from([("world", &vec)]))]);
    assert_eq!(
        serde_saphyr::to_string(&as_tup).unwrap(),
        serde_saphyr::to_string(&as_vec).unwrap(),
    );
}
