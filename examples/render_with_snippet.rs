use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Cfg {
    base_scalar: serde_saphyr::Spanned<u64>,
    key: Vec<usize>,
}

fn main() {
    // Intentionally invalid YAML to demonstrate snippet rendering.
    let yaml = r#"
    base_scalar: -z123 # this should be a number
    key: [ 1, 2, 2 ]
"#;

    let cfg: Result<Cfg, Error> = serde_saphyr::from_str(yaml);
    match cfg {
        Ok(cfg) => {
            // Keep the value used to avoid "unused" warnings if this example is copy-pasted.
            println!("{:?}", cfg);
        }
        Err(err) => {
            // By default, `from_str` wraps errors with snippet rendering.
            // Customize via `Options { with_snippet: false, .. }` if needed.
            eprintln!("{err}");
        }
    }
}
