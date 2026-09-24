use serde_saphyr::scalar::{ScalarError, ScalarKind, ScalarStyle, Schema, resolve};

const TIMESTAMP_TAG: &str = "tag:yaml.org,2002:timestamp";

fn assert_invalid_timestamp(text: &str) {
    // Invalid implicit timestamps remain strings, but an explicit timestamp tag
    // must report invalid content rather than silently falling back to a string.
    let scalar = resolve(text, ScalarStyle::Plain, None, Schema::Yaml11).unwrap();
    assert_eq!(scalar.kind(), ScalarKind::String, "{text:?}");
    assert_eq!(scalar.text(), text);

    for style in [
        ScalarStyle::Plain,
        ScalarStyle::SingleQuoted,
        ScalarStyle::DoubleQuoted,
    ] {
        assert_eq!(
            resolve(text, style, Some(TIMESTAMP_TAG), Schema::Yaml11).unwrap_err(),
            ScalarError::InvalidValue {
                kind: ScalarKind::Timestamp,
            },
            "{text:?}, {style:?}"
        );
    }
}

#[test]
fn timestamp_rejects_missing_overlong_and_nondigit_months() {
    for text in ["2001--15", "2001-123-15", "2001-x-15", "2001-１２-15"] {
        assert_invalid_timestamp(text);
    }
}

#[test]
fn timestamp_requires_separator_between_month_and_day() {
    for text in ["2001-12", "2001-12x15", "2001-12/15", "2001-12 15"] {
        assert_invalid_timestamp(text);
    }
}

#[test]
fn timestamp_rejects_missing_overlong_and_nondigit_days() {
    for text in ["2001-12-", "2001-12-123", "2001-12-x", "2001-12-１５"] {
        assert_invalid_timestamp(text);
    }
}

#[test]
fn timestamp_accepts_supported_date_widths_without_calendar_validation() {
    for text in [
        "2001-01-01",
        "2001-12-31",
        "2001-1-1T0:00:00",
        "2001-12-31T23:59:59Z",
        // Timestamp resolution validates spelling, not calendar ranges.
        "2001-99-99",
    ] {
        for tag in [None, Some(TIMESTAMP_TAG)] {
            let scalar = resolve(text, ScalarStyle::Plain, tag, Schema::Yaml11).unwrap();
            assert_eq!(scalar.kind(), ScalarKind::Timestamp, "{text:?}, {tag:?}");
            assert_eq!(scalar.text(), text);
        }
        for schema in [Schema::Strings, Schema::Yaml12] {
            let scalar = resolve(text, ScalarStyle::Plain, None, schema).unwrap();
            assert_eq!(scalar.kind(), ScalarKind::String, "{text:?}, {schema:?}");
        }
    }

    // One-digit month/day fields are allowed with a time, but not date-only.
    for text in ["2001-1-01", "2001-01-1", "2001-1-1"] {
        assert_invalid_timestamp(text);
    }
}
