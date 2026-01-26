use serde::Deserialize;
use serde::de::DeserializeOwned;

pub fn from_str<T: DeserializeOwned>(s: &str) -> Result<T, serde_saphyr::Error> {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::LastWins,
        strict_booleans: true,
        ignore_binary_tag_for_string: true,
        budget: Some(serde_saphyr::Budget {
            max_total_scalar_bytes: 65536,
            ..serde_saphyr::Budget::default()
        }),
    };
    serde_saphyr::from_str_with_options(s, options)
}

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
    enabled: bool,
    retries: i32,
}

fn main() -> anyhow::Result<()> {
    let yaml_input = r#"
  name: "My Application"
  enabled: true
  retries: 5
"#;
    let config: Config = from_str(yaml_input)?;
    println!(
        "Config: {}, {}, {}",
        config.name, config.enabled, config.retries
    );
    Ok(())
}
