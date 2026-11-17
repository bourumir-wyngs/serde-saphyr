// QB6E: Under-indented multiline quoted scalars require strict mode to reject.
use serde::Deserialize;
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource};

#[derive(Debug, Deserialize)]
struct Doc {
    quoted: String,
}

#[test]
fn yaml_qb6e_multiline_quoted_scalar_honors_strict_indentation() {
    let y = r#"---
quoted: "a
b
c""#;

    let doc: Doc = serde_saphyr::from_str(y).expect("relaxed mode accepts quoted continuations");
    assert_eq!(doc.quoted, "a b c");

    let error = serde_saphyr::from_str_with_options::<Doc>(
        y,
        serde_saphyr::options! { strict_indentation: true },
    )
    .expect_err("strict mode rejects under-indented quoted continuations");
    assert!(
        matches!(
            error.without_snippet(),
            Error::ExternalMessage { source, .. }
                if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                    if error.kind() == &ErrorKind::InvalidQuotedScalarIndent)
        ),
        "expected a quoted scalar indentation error, got {error}"
    );
}
