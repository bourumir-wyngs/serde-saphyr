use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Config {
    name: String,
    version: i32,
    enabled: bool,
    tags: Vec<String>,
    server: Server,
    users: Vec<User>,
    metadata: Metadata,
    notes: String,
    numbers: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Server {
    host: String,
    port: u16,
    tls: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct User {
    id: i32,
    name: String,
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Metadata {
    project: String,
    generated_by: String,
    generated_at: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg_figment = figment::Figment::from(
        <serde_saphyr::figment::Yaml as figment::providers::Format>::file("examples/value.yaml"),
    )
    .extract::<Config>()?;

    let cfg_figment2 = figment2::Figment::from(
        <serde_saphyr::figment2::Yaml as figment2::providers::Format>::file("examples/value.yaml"),
    )
    .extract::<Config>()?;

    println!("figment: {cfg_figment:?}");
    println!("figment2: {cfg_figment2:?}");
    Ok(())
}
