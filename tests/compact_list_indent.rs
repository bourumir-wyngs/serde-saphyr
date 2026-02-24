use serde::Serialize;
use serde_saphyr::{to_string, to_string_with_options, SerializerOptions};

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

    let yaml = to_string(&spec).unwrap();
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

    let opts = SerializerOptions {
        compact_list_indent: true,
        ..Default::default()
    };
    let yaml = to_string_with_options(&spec, opts).unwrap();
    let lines: Vec<&str> = yaml.lines().collect();
    assert_eq!(lines[0], "containers:");
    assert_eq!(lines[1], "- env:");
    assert_eq!(lines[2], "  - name: METHOD");
    assert_eq!(lines[3], "    value: WATCH");
}