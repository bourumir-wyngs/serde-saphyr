use serde::Deserialize;

#[test]
fn test_serde_fail() {
    #[derive(Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Temp {
        a: String,
        b: String,
        c: String,
    }
    let cfgstr =
        r###"
        a: "value a"
        b: "value b"
        c: "value c"
        x: "value x"
        "###;

    let cfg: Result<Temp, _> = serde_saphyr::from_str(cfgstr);
    let err = cfg.unwrap_err();
    let rendered = err.to_string();

    // The snippet should point at the unknown key `x`, not the container start.
    assert!(rendered.contains("unknown field `x`"), "{rendered}");
    assert!(rendered.contains("--> <input>:5:9"), "{rendered}");
}