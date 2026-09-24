//! The public macro constructs the non-exhaustive variant from optional fields.

use serde_saphyr::scalar::Schema;
use serde_saphyr::specific;

fn fields(schema: Schema) -> (bool, bool, bool) {
    match schema {
        Schema::Specific {
            strict_booleans,
            legacy_octal_numbers,
            quote_all,
            ..
        } => (strict_booleans, legacy_octal_numbers, quote_all),
        other => panic!("expected Specific, got {other:?}"),
    }
}

#[test]
fn omitted_fields_have_the_constructor_defaults() {
    assert_eq!(fields(Schema::specific()), (false, false, false));
    assert_eq!(specific! {}, Schema::specific());
    assert_eq!(specific!(), Schema::specific());
}

#[test]
fn each_field_can_be_selected_independently() {
    assert_eq!(
        fields(specific! { strict_booleans: true }),
        (true, false, false),
    );
    assert_eq!(
        fields(specific! { legacy_octal_numbers: true, }),
        (false, true, false),
    );
    assert_eq!(fields(specific! { quote_all: true }), (false, false, true),);
}

#[test]
fn fields_may_be_combined_in_any_order() {
    assert_eq!(
        fields(specific! {
            quote_all: true,
            strict_booleans: true,
            legacy_octal_numbers: true,
        }),
        (true, true, true),
    );
    assert_eq!(
        fields(specific! {
            legacy_octal_numbers: true,
            quote_all: false,
            strict_booleans: true
        }),
        (true, true, false),
    );
}

#[test]
fn repeated_fields_keep_the_last_value() {
    assert_eq!(
        fields(specific! {
            strict_booleans: true,
            quote_all: true,
            strict_booleans: false,
            legacy_octal_numbers: false,
            legacy_octal_numbers: true,
            quote_all: false,
        }),
        (false, true, false),
    );
}

#[test]
fn field_expressions_are_evaluated_once_in_supplied_order() {
    let mut trace = Vec::new();
    let schema = specific! {
        quote_all: { trace.push("quote-first"); true },
        strict_booleans: { trace.push("strict"); true },
        legacy_octal_numbers: { trace.push("octal"); false },
        quote_all: { trace.push("quote-last"); false },
    };
    assert_eq!(trace, ["quote-first", "strict", "octal", "quote-last"]);
    assert_eq!(fields(schema), (true, false, false));
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
    };
    assert_eq!(fields(result), (true, false, true));
    assert!(schema);
    assert!(!strict_booleans);
    assert!(value);
}

#[test]
fn constructor_and_macro_work_in_const_contexts() {
    const DEFAULT: Schema = Schema::specific();
    const CUSTOM: Schema = specific! { strict_booleans: true, quote_all: true };

    assert_eq!(fields(DEFAULT), (false, false, false));
    assert_eq!(fields(CUSTOM), (true, false, true));
}

#[cfg(feature = "deserialize")]
#[test]
fn macro_nests_in_deserializer_options() {
    let options = serde_saphyr::options! {
        schema: specific! { strict_booleans: true, legacy_octal_numbers: true },
    };
    assert_eq!(fields(options.schema), (true, true, false));
}

#[cfg(feature = "serialize")]
#[test]
fn macro_nests_in_serializer_options() {
    let options = serde_saphyr::ser_options! {
        schema: specific! { quote_all: true },
    };
    assert_eq!(fields(options.schema), (false, false, true));
}
