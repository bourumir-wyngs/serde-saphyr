/*
  ----
  Show how to enrich YAML deserialization errors with a Serde *path* using
  the `serde_path_to_error` crate.

  Why?
  ------------------------
  When parsing nested config structures, a type mismatch error like
  "invalid type: string \"oops\", expected u16" is hard to act on without
  knowing *where* it happened. `serde_path_to_error` wraps a Serde deserializer and
  records the access path so you can report it alongside the underlying error.

  While serde-saphyr has its own diagnostics, this expample shows how to use serde-saphyr
  deserializer with `serde_path_to_error`.
*/

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Config {
    server: Server,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Server {
    host: String,
    port: u16,
}

fn main() {
    // `port` is intentionally invalid: it's a string but we expect `u16`.
    let yaml = r#"
        server:
          host: localhost
          port: "oops"
    "#;

    let res: Result<Config, serde_saphyr::Error> = serde_saphyr::with_deserializer_from_str(
        yaml,
        |de| match serde_path_to_error::deserialize::<_, Config>(de) {
            Ok(v) => Ok(v),
            Err(e) => Err(<serde_saphyr::Error as serde::de::Error>::custom(format!(
                "deserialization error at {}: {}",
                e.path(),
                e
            ))),
        },
    );

    match res {
        Ok(cfg) => println!("Parsed config: {cfg:?}"),
        Err(e) => eprintln!("{e}"),
    }
}
