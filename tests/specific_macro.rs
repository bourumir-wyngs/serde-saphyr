//! The public macro constructs the non-exhaustive variant from optional fields.

use serde_saphyr::scalar::Schema;
use serde_saphyr::specific;

fn fields(schema: Schema) -> (bool, bool, bool, bool) {
    match schema {
        Schema::Specific {
            strict_booleans,
            legacy_octal_numbers,
            quote_all,
            yaml_12_quoting,
            ..
        } => (
            strict_booleans,
            legacy_octal_numbers,
            quote_all,
            yaml_12_quoting,
        ),
        other => panic!("expected Specific, got {other:?}"),
    }
}

#[test]
fn omitted_fields_have_the_constructor_defaults() {
    assert_eq!(fields(Schema::specific()), (false, false, false, false));
    assert_eq!(specific! {}, Schema::specific());
    assert_eq!(specific!(), Schema::specific());
    // Compare complete schemas as well, so newly added fields retain the
    // constructor's defaults even when the macro overrides another field.
    for schema in [
        specific! { strict_booleans: false },
        specific! { legacy_octal_numbers: false },
        specific! { quote_all: false },
        specific! { yaml_12_quoting: false },
    ] {
        assert_eq!(schema, Schema::specific());
    }
}

#[test]
fn each_field_can_be_selected_independently() {
    assert_eq!(
        fields(specific! { strict_booleans: true }),
        (true, false, false, false),
    );
    assert_eq!(
        fields(specific! { legacy_octal_numbers: true, }),
        (false, true, false, false),
    );
    assert_eq!(
        fields(specific! { quote_all: true }),
        (false, false, true, false),
    );
    assert_eq!(
        fields(specific! { yaml_12_quoting: true }),
        (false, false, false, true),
    );
}

#[test]
fn fields_may_be_combined_in_any_order() {
    assert_eq!(
        fields(specific! {
            quote_all: true,
            strict_booleans: true,
            legacy_octal_numbers: true,
            yaml_12_quoting: true,
        }),
        (true, true, true, true),
    );
    assert_eq!(
        fields(specific! {
            legacy_octal_numbers: true,
            yaml_12_quoting: false,
            quote_all: false,
            strict_booleans: true
        }),
        (true, true, false, false),
    );
}

#[test]
fn repeated_fields_keep_the_last_value() {
    assert_eq!(
        fields(specific! {
            strict_booleans: true,
            quote_all: true,
            yaml_12_quoting: true,
            strict_booleans: false,
            legacy_octal_numbers: false,
            legacy_octal_numbers: true,
            quote_all: false,
            yaml_12_quoting: false,
        }),
        (false, true, false, false),
    );
}

#[test]
fn field_expressions_are_evaluated_once_in_supplied_order() {
    let mut trace = Vec::new();
    let schema = specific! {
        quote_all: { trace.push("quote-first"); true },
        yaml_12_quoting: { trace.push("yaml12"); true },
        strict_booleans: { trace.push("strict"); true },
        legacy_octal_numbers: { trace.push("octal"); false },
        quote_all: { trace.push("quote-last"); false },
    };
    assert_eq!(
        trace,
        ["quote-first", "yaml12", "strict", "octal", "quote-last"]
    );
    assert_eq!(fields(schema), (true, false, false, true));
}

#[test]
fn macro_is_hygienic_through_a_renamed_crate_import() {
    use serde_saphyr as renamed;

    let schema = true;
    let strict_booleans = false;
    let value = true;
    let result = renamed::specific! {
        strict_booleans: schema,
        legacy_octal_numbers: strict_booleans,
        quote_all: value,
        yaml_12_quoting: schema,
    };
    assert_eq!(fields(result), (true, false, true, true));
    assert!(schema);
    assert!(!strict_booleans);
    assert!(value);
}

#[test]
fn constructor_and_macro_work_in_const_contexts() {
    const DEFAULT: Schema = Schema::specific();
    const CUSTOM: Schema = specific! {
        strict_booleans: true,
        quote_all: true,
        yaml_12_quoting: true,
    };

    assert_eq!(fields(DEFAULT), (false, false, false, false));
    assert_eq!(fields(CUSTOM), (true, false, true, true));
}

#[cfg(feature = "deserialize")]
#[test]
fn macro_nests_in_deserializer_options() {
    let options = serde_saphyr::options! {
        schema: specific! { strict_booleans: true, legacy_octal_numbers: true },
    };
    assert_eq!(fields(options.schema), (true, true, false, false));
}

#[cfg(feature = "serialize")]
#[test]
fn macro_nests_in_serializer_options() {
    let options = serde_saphyr::ser_options! {
        schema: specific! { quote_all: true, yaml_12_quoting: true },
    };
    assert_eq!(fields(options.schema), (false, false, true, true));
}
