use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Cfg {
    key: Vec<usize>,
}

// see https://github.com/j178/prek/issues/1111
fn main() {
    let yaml = r#"
    key: [ 1, 2, 2
    ] # this sits wrongly but okay
"#;

    let cfg: Result<Cfg, Error> = serde_saphyr::from_str(yaml);
    match cfg {
        Ok(cfg) => {
            println!("Fine: {:?}", cfg); // Fine: Cfg { key: [1, 2, 2] }
        }
        Err(err) => {
            eprintln!("{err}");
        }
    }
}
