use std::collections::HashMap;

#[test]
fn yaml12_emits_directive_and_does_not_quote_yaml11_boolean_spellings() {
    let options = serde_saphyr::ser_options! { yaml_12: true };

    // Use YAML 1.1 boolean spellings as STRING keys.
    let mut map = HashMap::new();
    map.insert("y", 0);
    map.insert("n", 1);
    map.insert("yes", 2);
    map.insert("no", 3);
    map.insert("on", 4);
    map.insert("off", 5);
    map.insert("true", 6);
    map.insert("0", 7);

    let mut out = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut out, &map, options).unwrap();

    assert!(
        out.starts_with("%YAML 1.2\n"),
        "missing YAML 1.2 directive: {out}"
    );

    // YAML 1.1-only boolean spellings should not be auto-quoted under yaml_12.
    assert!(
        out.contains("\ny:"),
        "y key should be plain under yaml_12: {out}"
    );
    assert!(
        out.contains("\nn:"),
        "n key should be plain under yaml_12: {out}"
    );
    assert!(
        out.contains("\nyes:"),
        "yes key should be plain under yaml_12: {out}"
    );
    assert!(
        out.contains("\nno:"),
        "no key should be plain under yaml_12: {out}"
    );
    assert!(
        out.contains("\non:"),
        "on key should be plain under yaml_12: {out}"
    );
    assert!(
        out.contains("\noff:"),
        "off key should be plain under yaml_12: {out}"
    );

    assert!(
        !out.contains("\"y\":"),
        "y key should not be quoted under yaml_12: {out}"
    );
    assert!(
        !out.contains("\"n\":"),
        "n key should not be quoted under yaml_12: {out}"
    );
    assert!(
        out.contains("\"true\":"),
        "true is a YAML 1.2 boolean literal and must stay quoted as a string key: {out}"
    );
    assert!(
        out.contains("\"0\":"),
        "numeric-looking string key must stay quoted: {out}"
    );
}

#[test]
fn yaml12_disables_auto_quoting_of_yaml11_boolean_spellings_in_values() {
    let mut out_default = String::new();
    serde_saphyr::to_fmt_writer_with_options(
        &mut out_default,
        &HashMap::from([("k", "yes")]),
        serde_saphyr::SerializerOptions::default(),
    )
    .unwrap();
    assert!(
        out_default.contains("k: \"yes\""),
        "default mode should quote YAML 1.1 boolean spellings in values: {out_default}"
    );

    let mut out_yaml12 = String::new();
    serde_saphyr::to_fmt_writer_with_options(
        &mut out_yaml12,
        &HashMap::from([("k", "yes")]),
        serde_saphyr::ser_options! { yaml_12: true },
    )
    .unwrap();

    assert!(
        out_yaml12.starts_with("%YAML 1.2\n"),
        "missing YAML 1.2 directive: {out_yaml12}"
    );
    assert!(
        out_yaml12.contains("k: yes"),
        "yaml_12 mode should not quote YAML 1.1 boolean spellings in values: {out_yaml12}"
    );
    assert!(
        !out_yaml12.contains("k: \"yes\""),
        "yaml_12 mode should not quote: {out_yaml12}"
    );
}
