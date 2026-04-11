#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(feature = "include")]

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Root {
    foo: String,
}

#[test]
fn test_multi_doc_include() {
    let yaml = "foo: !include multi.yaml\n";
    let options = serde_saphyr::options! {};
    let options = options.with_include_resolver(|req| {
        Ok(serde_saphyr::ResolvedInclude {
            id: req.spec.to_string(),
            name: req.spec.to_string(),
            source: serde_saphyr::InputSource::from_string("doc1\n---\ndoc2\n".to_string()),
        })
    });

    let err = serde_saphyr::from_str_with_options::<Root>(yaml, options)
        .expect_err("multi-document include must fail for single-document API");

    let rendered = err.to_string();
    assert!(
        rendered.contains("multiple documents") || rendered.contains("More than one YAML document"),
        "expected multiple-documents failure mode, got: {rendered}"
    );
}
