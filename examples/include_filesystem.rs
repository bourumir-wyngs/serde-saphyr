use serde::Deserialize;
use serde_saphyr::{from_str_with_options, options};

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    selected_users: Vec<User>,
    repeated_users: Vec<User>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct User {
    id: u32,
    name: String,
    roles: Vec<String>,
}

fn main() {
    let yaml = r#"
selected_users: &users !include#users value.yaml
repeated_users: *users
"#;

    let options = options! {}
        // Assuming we are running in a project root folder where "examples" is subfolder.
        .with_filesystem_root("examples")
        .expect("failed to create filesystem include resolver");

    let config: Config =
        from_str_with_options(yaml, options).expect("failed to parse filesystem include example");

    println!("Parsed configuration:\n{config:#?}");

    assert_eq!(config.selected_users, config.repeated_users);
    assert_eq!(config.selected_users.len(), 2);
    assert_eq!(config.selected_users[0].name, "Alice");
    assert_eq!(config.selected_users[1].name, "Bob");
}