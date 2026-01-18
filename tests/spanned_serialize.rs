use serde::{Deserialize, Serialize};

use serde_saphyr::{Spanned, from_str, to_string};

#[test]
fn struct_containing_spanned_can_serialize_value_only() {
    #[derive(Debug, Deserialize, Serialize)]
    struct Cfg {
        timeout: Spanned<u64>,
    }

    let cfg: Cfg = from_str("timeout: 5\n").unwrap();

    let out = to_string(&cfg).unwrap();

    assert!(out.contains("timeout: 5"), "unexpected yaml output: {out}");
    assert!(
        !out.contains("referenced") && !out.contains("defined"),
        "Spanned metadata must not be serialized: {out}"
    );
}
