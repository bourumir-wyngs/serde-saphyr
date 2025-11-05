#[test]
fn test_quoting() {
    let samples = vec![
        "2413",
        "2_4_13",
        "0x11",
        "0o11",
        "0b11",
        "1.1",
        ".1",
        "1e3",
        ".inf",
        ".nan",
    ];
    for v in samples {
        let s = serde_saphyr::to_string(&v).unwrap();
        let vv = serde_saphyr::from_str::<serde_json::Value>(&s).unwrap();
        assert!(vv.is_string(), "{} is {}", s, vv);
    }
}