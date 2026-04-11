use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, PartialEq)]
struct Config {
    namespaces: HashMap<String, Option<()>>,
    #[serde(rename = "helmRepos")]
    helm_repos: HashMap<String, String>,
    context: String,
    apps: HashMap<String, App>,
}

#[derive(Deserialize, Debug, PartialEq)]
struct App {
    enabled: bool,
    namespace: String,
    chart: String,
    version: String,
}

#[test]
fn variable_yaml() {
    let yaml = r#"
namespaces:
  production:

helmRepos:
  stable: "https://kubernetes-charts.storage.googleapis.ch"

context: monitoring

apps:
  datahog:
    enabled: true
    namespace: production
    chart: stable/datahog
    version: "1.38.7"
    "#;

    let config: Config = serde_saphyr::from_str(yaml).unwrap();

    let mut namespaces = HashMap::new();
    namespaces.insert("production".to_string(), None);

    let mut helm_repos = HashMap::new();
    helm_repos.insert(
        "stable".to_string(),
        "https://kubernetes-charts.storage.googleapis.ch".to_string(),
    );

    let mut apps = HashMap::new();
    apps.insert(
        "datahog".to_string(),
        App {
            enabled: true,
            namespace: "production".to_string(),
            chart: "stable/datahog".to_string(),
            version: "1.38.7".to_string(),
        },
    );

    let expected = Config {
        namespaces,
        helm_repos,
        context: "monitoring".to_string(),
        apps,
    };

    assert_eq!(config, expected);
}
