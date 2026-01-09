#[test]
fn serialize_small_float_scientific_notation() {
    let value = 0.000004_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e-6");

    let value = 4.5123456e-18_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.5123456e-18");
}

#[test]
fn serialize_large_float_scientific_notation() {
    let value = 40000000000000000000.0_f64;
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml.trim(), "4.0e+19");
}

#[test]
fn roundtrip_floats() {
    for original in [4.0e-6, 3.12e18, 17.4] {
        let yaml = serde_saphyr::to_string(&original).unwrap();
        let parsed: f64 = serde_saphyr::from_str(&yaml).unwrap();
        assert_eq!(original, parsed);
    }
}
