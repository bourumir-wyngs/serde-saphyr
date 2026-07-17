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
        Ok(serde_saphyr::ResolvedInclude::new(
            req.spec,
            req.spec,
            serde_saphyr::InputSource::from_string("doc1\n---\ndoc2\n".to_string()),
        ))
    });

    let err = serde_saphyr::from_str_with_options::<Root>(yaml, options)
        .expect_err("multi-document include must fail for single-document API");

    assert!(matches!(
        err.without_snippet(),
        serde_saphyr::Error::MultipleDocuments { .. }
    ));
}
