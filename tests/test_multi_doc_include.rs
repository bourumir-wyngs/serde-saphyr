#![cfg(feature = "include")]

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Root {
    foo: String,
}

#[test]
fn test_multi_doc_include() {
    let yaml = "foo: !include multi.yaml\n";
    let options = serde_saphyr::Options::default();
    let options = options.with_include_resolver(|req| {
        Ok(serde_saphyr::ResolvedInclude {
            id: req.spec.to_string(),
            name: req.spec.to_string(),
            source: serde_saphyr::InputSource::from_string("doc1\n---\ndoc2\n".to_string()),
        })
    });
    
    let res: Result<Root, _> = serde_saphyr::from_str_with_options(yaml, options);
    println!("Result: {:#?}", res);
    assert!(res.is_err());
}