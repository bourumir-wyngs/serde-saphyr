use serde::Deserialize;

#[test]
fn from_slice_with_options_supports_borrowed_str() {
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Cfg<'a> {
        name: &'a str,
    }

    let yaml = "name: hello\n";
    let bytes = yaml.as_bytes();

    let cfg: Cfg<'_> =
        serde_saphyr::from_slice_with_options(bytes, serde_saphyr::Options::default())
            .expect("deserialize borrowed config");

    assert_eq!(cfg.name, "hello");
}
