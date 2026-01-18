use serde::Deserialize;

#[derive(Deserialize)]
struct UrlEntry {
    url: String,
}

#[test]
fn y_udm2_plain_url_in_flow_mapping() {
    let yaml = "- { url: http://example.org }\n";

    let v: Vec<UrlEntry> = serde_saphyr::from_str(yaml).expect("failed to parse UDM2 YAML");

    assert_eq!(v.len(), 1);
    assert_eq!(v[0].url, "http://example.org");
}
