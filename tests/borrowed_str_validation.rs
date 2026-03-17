use serde::Deserialize;
use serde_saphyr::from_str_with_options;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BorrowedStr<'a> {
    s: &'a str,
}

#[test]
fn borrowed_str_respects_no_schema_quoting_rules() {
    let yaml = "s: true\n";
    let opts = serde_saphyr::options! { no_schema: true };

    let err = from_str_with_options::<BorrowedStr<'_>>(yaml, opts).unwrap_err();
    assert!(
        err.to_string().contains("must be quoted"),
        "unexpected error: {err}"
    );
}

#[test]
fn borrowed_str_does_not_accept_raw_binary_payload() {
    let yaml = "s: !!binary aGVsbG8=\n";

    let err = serde_saphyr::from_str::<BorrowedStr<'_>>(yaml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("borrowed string") || msg.contains("String or Cow"),
        "unexpected error: {msg}"
    );
}
