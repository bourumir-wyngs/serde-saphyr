use serde_json::Value;

#[test]
fn require_indent_even_accepts_even_indentation() {
    let yaml = "root:\n  child: value\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Even,
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_ok(),
        "Even indentation (2) should be accepted: {result:?}"
    );
}

#[test]
fn require_indent_even_rejects_odd_indentation() {
    let yaml = "root:\n   child: value\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Even,
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(result.is_err(), "Odd indentation (3) should be rejected");
}

#[test]
fn require_indent_divisible_by_4_accepts_4() {
    let yaml = "root:\n    child: value\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Divisible(4),
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_ok(),
        "Indentation of 4 should be accepted by Divisible(4): {result:?}"
    );
}

#[test]
fn require_indent_divisible_by_4_rejects_2() {
    let yaml = "root:\n  child: value\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Divisible(4),
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_err(),
        "Indentation of 2 should be rejected by Divisible(4)"
    );
}

#[test]
fn require_indent_uniform_accepts_consistent_indentation() {
    let yaml = "a:\n  b: 1\n  c: 2\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Uniform(None),
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_ok(),
        "Consistent 2-space indentation should be accepted: {result:?}"
    );
}

#[test]
fn require_indent_uniform_rejects_mixed_indentation() {
    // First level uses 2 spaces, second level uses 3 (not a multiple of 2).
    let yaml = "a:\n  b:\n     c: 1\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Uniform(None),
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_err(),
        "Mixed indentation (2 then 3) should be rejected by Uniform"
    );
}

#[test]
fn require_indent_unchecked_accepts_anything() {
    let yaml = "a:\n   b:\n       c: 1\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Unchecked,
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_ok(),
        "Unchecked should accept any indentation: {result:?}"
    );
}

#[test]
fn require_indent_error_is_indentation_error() {
    let yaml = "root:\n   child: value\n";
    let options = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Even,
    };
    let err = serde_saphyr::from_str_with_options::<Value>(yaml, options).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("indentation"),
        "Error message should mention indentation: {msg}"
    );
}

#[test]
fn require_indent_default_is_unchecked() {
    let options = serde_saphyr::options! {};
    // Odd indentation should pass with default (Unchecked)
    let yaml = "a:\n   b: 1\n";
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(
        result.is_ok(),
        "Default (Unchecked) should accept any indentation: {result:?}"
    );
}
