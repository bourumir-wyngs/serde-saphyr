#![cfg(all(feature = "serialize", feature = "deserialize", feature = "include"))]
use serde_json::Value;
use serde_saphyr::RequireIndent;

fn parse_with_include(
    require: RequireIndent,
    main: &str,
    child: &'static str,
) -> Result<Value, String> {
    let options = serde_saphyr::options! { require_indent: require };
    let options = options.with_include_resolver(move |req| {
        Ok(serde_saphyr::ResolvedInclude::new(
            req.spec,
            req.spec,
            serde_saphyr::InputSource::from_string(child.to_string()),
        ))
    });
    serde_saphyr::from_str_with_options::<Value>(main, options).map_err(|e| e.to_string())
}

#[test]
fn uniform_some_rejects_inconsistent_included_indentation() {
    let err = parse_with_include(
        RequireIndent::Uniform(Some(2)),
        "root: !include child.yaml",
        "a:\n   b: 1",
    )
    .unwrap_err();
    assert!(
        err.contains("expected uniform (2 spaces), found 3 spaces"),
        "{err}"
    );
}

#[test]
fn uniform_some_accepts_consistent_included_indentation() {
    let result = parse_with_include(
        RequireIndent::Uniform(Some(2)),
        "root: !include child.yaml",
        "a:\n  b: 1",
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn uniform_none_inferred_unit_carries_into_included_document() {
    let err = parse_with_include(
        RequireIndent::Uniform(None),
        "parent:\n  child: !include child.yaml",
        "a:\n   b: 1",
    )
    .unwrap_err();
    assert!(
        err.contains("expected uniform (2 spaces), found 3 spaces"),
        "{err}"
    );
}
