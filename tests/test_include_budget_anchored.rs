#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(feature = "include")]

use serde::Deserialize;
use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude, from_reader_with_options};

#[derive(Debug, Deserialize, PartialEq)]
struct Root {
    included1: String,
    included2: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct ExactFitRoot {
    pad: String,
    included: String,
}

#[test]
fn test_anchored_includes_exceed_budget() {
    let yaml = r#"
i1: !include "f.yml#f"
i2: !include "f.yml#f"
"#;
    let anchored_text = format!("root: &f |\n  {}\n", "a".repeat(80));

    // root YAML ~40 bytes
    // anchored include payload ~92 bytes
    // Total needed > 220 with two includes
    // Limit: 150
    // First include should pass, second include should fail.
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(150),
        },
    }
    .with_include_resolver(move |req| {
        assert_eq!(req.spec, "f.yml#f");
        Ok(ResolvedInclude {
            id: req.spec.to_string(),
            name: req.spec.to_string(),
            source: InputSource::AnchoredText {
                text: anchored_text.clone(),
                anchor: "f".to_string(),
            },
        })
    });

    let result: Result<Root, _> = from_reader_with_options(yaml.as_bytes(), options);
    assert!(
        result.is_err(),
        "Expected parsing to fail due to budget exhaustion"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("exceed"),
        "Error should mention exceeding limit, got: {}",
        err_msg
    );
}

#[test]
fn test_anchored_include_succeeds_when_fragment_exactly_fits_remaining_budget() {
    let pad = "1234567890";
    let yaml = format!("pad: {pad}\nincluded: !include \"f.yml#f\"\n");
    let anchored_text = "root: &f |
  exactly_twenty_bytes\n";
    let resolver =
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            assert_eq!(req.spec, "f.yml#f");
            Ok(ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: InputSource::AnchoredText {
                    text: anchored_text.to_string(),
                    anchor: "f".to_string(),
                },
            })
        };
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(yaml.len() + anchored_text.len()),
        },
    }
    .with_include_resolver(resolver);

    let parsed: ExactFitRoot = from_reader_with_options(yaml.as_bytes(), options)
        .expect("anchored include should succeed when it exactly fits the remaining reader budget");

    assert_eq!(parsed.pad, pad);
    assert_eq!(parsed.included, "exactly_twenty_bytes\n");
}

#[test]
fn test_same_anchored_include_parses_with_different_limits() {
    let yaml = b"included1: !include \"f.yml#f\"\nincluded2: !include \"f.yml#f\"\n";
    let anchored_text = format!("root: &f |\n  {}\n", "a".repeat(80));
    let anchored_text_len = anchored_text.len();
    let resolver_ok =
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            assert_eq!(req.spec, "f.yml#f");
            Ok(ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: InputSource::AnchoredText {
                    text: anchored_text.clone(),
                    anchor: "f".to_string(),
                },
            })
        };
    let options_ok = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(yaml.len() + (2 * anchored_text_len)),
        },
    }
    .with_include_resolver(resolver_ok);

    let parsed: Root = from_reader_with_options(std::io::Cursor::new(yaml), options_ok)
        .expect("same anchored input should parse when the combined budget is sufficient");
    assert_eq!(parsed.included1, format!("{}\n", "a".repeat(80)));
    assert_eq!(parsed.included2, format!("{}\n", "a".repeat(80)));

    let anchored_text = format!("root: &f |\n  {}\n", "a".repeat(80));
    let anchored_text_len = anchored_text.len();
    let resolver_err =
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            assert_eq!(req.spec, "f.yml#f");
            Ok(ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: InputSource::AnchoredText {
                    text: anchored_text.clone(),
                    anchor: "f".to_string(),
                },
            })
        };
    let options_err = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(yaml.len() + (2 * anchored_text_len) - 1),
        },
    }
    .with_include_resolver(resolver_err);

    let err = from_reader_with_options::<_, Root>(std::io::Cursor::new(yaml), options_err)
        .expect_err("same anchored input should fail when the combined budget is too small");
    assert!(err.to_string().contains("exceed"));
}
