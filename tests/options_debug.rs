#![cfg(all(feature = "serialize", feature = "deserialize"))]

#[cfg(all(feature = "include", feature = "properties"))]
#[test]
fn options_debug_reports_set_include_resolver_and_properties() {
    let mut properties = std::collections::HashMap::new();
    properties.insert("MODE".to_owned(), "test".to_owned());

    let options = serde_saphyr::Options::default()
        .with_properties(properties)
        .with_include_resolver(|_req| {
            Err(serde_saphyr::IncludeResolveError::Message(
                "not used by this test".to_owned(),
            ))
        });

    let debug = format!("{options:?}");
    assert!(debug.contains("include_resolver: \"set\""), "{debug}");
    assert!(debug.contains("property_map: \"set\""), "{debug}");
}
