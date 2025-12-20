use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Cfg {
    base_scalar: serde_saphyr::Spanned<u64>,
    x: serde_saphyr::Spanned<u64>,
}

fn main() {
    let yaml = r#"
    base_scalar: &a 123 # we define it at line 2
    x: *a               # we reference it at line 3
"#;

    let cfg: Cfg = serde_saphyr::from_str(yaml).unwrap();

    println!(
        "base_scalar = {} (referenced {:?}, defined {:?})",
        cfg.base_scalar.value, cfg.base_scalar.referenced, cfg.base_scalar.defined
    );
    println!(
        "x = {} (referenced {:?}, defined {:?})",
        cfg.x.value, cfg.x.referenced, cfg.x.defined
    );

    assert_eq!(cfg.base_scalar.value, 123);
    assert_eq!(cfg.base_scalar.referenced, cfg.base_scalar.defined);

    assert_eq!(cfg.x.value, 123);
    assert_eq!(cfg.x.referenced.line(), 3);
    assert_eq!(cfg.x.referenced.column(), 8);
    assert_eq!(cfg.x.defined.line(), 2);
    assert_eq!(cfg.x.defined.column(), 21);
}
