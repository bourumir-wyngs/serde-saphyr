use figment::{Figment, providers::Format};
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
    let cfg = Figment::from(serde_saphyr::figment::Yaml::file("examples/value.yaml"))
        .extract::<Config>()?;
    println!("{cfg:?}");
    Ok(())
}
