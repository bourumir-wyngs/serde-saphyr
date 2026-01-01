/*
  examples/serde_ignored.rs

  Goal
  ----
  Show how to report *unknown / ignored* fields while deserializing YAML.

  Why would you want this?
  ------------------------
  When you deserialize into a Rust struct, Serde normally ignores fields that
  are present in the input but not present in your struct (unless you opt into
  `#[serde(deny_unknown_fields)]`, which turns unknown fields into a hard error).

  In configuration files, a *warning* is often the best behavior:
    - It helps users spot typos ("enabeld" instead of "enabled").
    - It helps detect stale config knobs after upgrades.
    - It keeps backwards/forwards compatibility (unknown fields don't fail the load).

  The `serde_ignored` crate provides this warning behavior by wrapping a Serde
  deserializer and calling you back for every ignored path.

  This example uses:
    - `serde_saphyr::with_deserializer_from_str` to obtain a streaming YAML deserializer
      (you cannot return the deserializer directly because it borrows parsing state).
    - `serde_ignored::deserialize` to collect the ignored fields.

  Running
  -------
  cargo run --example serde_ignored
*/

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    enabled: bool,
    retries: i32,
    server: Server,
}

#[derive(Debug, Deserialize)]
struct Server {
    host: String,
    port: u16,
}

fn main() {
    // Input YAML deliberately contains a few unknown fields.
    //
    // Notes:
    // - `enabeld` is a typo and will be ignored (because the struct expects `enabled`).
    // - `server.timeoutMs` does not exist in `Server` and will be ignored.
    // - `topLevelExtra` is not in `Config` and will be ignored.
    let yaml = r#"
        enabled: true
        retries: 5

        # typo: should be `enabled`
        enabeld: false

        server:
          host: localhost
          port: 8080
          timeoutMs: 250

        topLevelExtra: "surprise"
    "#;

    // Collect ignored paths as strings so we can print them later.
    //
    // `serde_ignored` reports paths in a structured form; converting to string
    // is convenient for human-facing diagnostics.
    let mut ignored = Vec::<String>::new();

    let cfg: Config = serde_saphyr::with_deserializer_from_str(yaml, |de| {
        // `serde_ignored::deserialize` behaves like `T::deserialize(de)`, but it
        // also calls our callback for every ignored path.
        serde_ignored::deserialize(de, |path| ignored.push(path.to_string()))
    })
    .expect("YAML input must deserialize into Config");

    // Your app can now decide how to surface ignored fields.
    //
    // Typical policies are:
    // - print warnings (most common)
    // - log warnings with file/line context
    // - treat some prefixes as errors
    if !ignored.is_empty() {
        eprintln!("Ignored (unknown) fields:");
        for p in &ignored {
            eprintln!("  - {p}");
        }
    }

    println!("Parsed config: {cfg:?}");
}
