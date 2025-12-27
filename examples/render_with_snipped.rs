use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Cfg {
    base_scalar: serde_saphyr::Spanned<u64>,
    key: Vec<usize>
}

fn main() {
    // Intentionally invalid YAML to demonstrate snippet rendering.
    //  Move closing bracket under "key" to result a valid YAML.
    let yaml = r#"
    base_scalar: -z123
    key: [ 1, 2, 2 ]
"#;

    let cfg: Result<Cfg, Error> = serde_saphyr::from_str(yaml);
    match cfg {
        Ok(cfg) => {
            // Keep the value used to avoid "unused" warnings if this example is copy-pasted.
            println!("{:?}", cfg);

        }
        Err(err) => {
            eprintln!("{}", err.with_snippet(yaml));
        }
    }
}