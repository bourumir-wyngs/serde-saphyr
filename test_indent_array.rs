// Test for indent_array option
// This demonstrates Go-like YAML behavior: arrays with 2-space indent, objects with 4-space indent

use serde::Serialize;
use serde_saphyr::{to_fmt_writer_with_options, SerializerOptions};

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

fn main() {
    let config = Config {
        name: "test-config".to_string(),
        servers: vec![
            Server {
                host: "localhost".to_string(),
                port: 8080,
            },
            Server {
                host: "example.com".to_string(),
                port: 9090,
            },
        ],
        settings: Settings {
            timeout: 30,
            retry: true,
        },
    };

    // Default behavior (both use indent_step = 2)
    println!("=== Default (indent_step: 2) ===");
    let mut buf = String::new();
    to_fmt_writer_with_options(&mut buf, &config, SerializerOptions::default()).unwrap();
    println!("{}", buf);

    // Go-like behavior (indent_array: 2, indent_step: 4)
    println!("\n=== Go-like (indent_step: 4, indent_array: 2) ===");
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
    println!("{}", buf);

    // Array with 3 spaces, objects with 2 spaces
    println!("\n=== Custom (indent_step: 2, indent_array: 3) ===");
    let mut buf = String::new();
    to_fmt_writer_with_options(
        &mut buf,
        &config,
        SerializerOptions {
            indent_step: 2,
            indent_array: Some(3),
            ..Default::default()
        },
    )
    .unwrap();
    println!("{}", buf);
}

