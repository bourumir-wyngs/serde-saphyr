//! Moving legacy flags into Specific must preserve byte-for-byte YAML output.
#![cfg(feature = "serialize")]
#![allow(deprecated)] // The old options are the compatibility reference.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_saphyr::{
    DoubleQuoted, FlowMap, FlowSeq, FoldStr, LitStr, SerializerOptions, SingleQuoted, ser_options,
    specific, to_string_multiple_with_options, to_string_with_options,
};

fn configurations() -> Vec<(SerializerOptions, SerializerOptions)> {
    let mut configurations = Vec::new();
    for yaml_12 in [false, true] {
        for quote_all in [false, true] {
            for strict_booleans in [false, true] {
                for legacy_octal_numbers in [false, true] {
                    let old = ser_options! { yaml_12: yaml_12, quote_all: quote_all };
                    let migrated = ser_options! {
                        schema: specific! {
                            yaml_12_quoting: yaml_12,
                            quote_all: quote_all,
                            strict_booleans: strict_booleans,
                            legacy_octal_numbers: legacy_octal_numbers,
                        },
                    };
                    configurations.push((old, migrated));
                }
            }
        }
    }
    configurations
}

fn assert_same_output<T: Serialize>(
    value: &T,
    old: &SerializerOptions,
    migrated: &SerializerOptions,
) {
    assert_eq!(
        to_string_with_options(value, migrated.clone()).unwrap(),
        to_string_with_options(value, old.clone()).unwrap(),
        "migrated options: {migrated:?}"
    );
}

#[test]
fn legacy_and_specific_match_for_strings_and_mapping_keys() {
    let strings = [
        "word",
        "yes",
        "OFF",
        "TrUe",
        "true",
        "null",
        "~",
        "2001-12-15",
        "2001-12-15T02:59:43Z",
        "inf",
        ".Inf",
        "NaN",
        "42",
        "010",
        "0x_10",
        "0x10",
        "0o10",
        "1_000",
        "1:20",
        "1e2",
        "1e+2",
        ".5",
        "<<",
        "=",
        "---",
        "",
        " leading space",
        "trailing space ",
        "needs: quoting",
        "inline # comment",
        "inline#hash",
        "comma,value",
        "[brackets]",
        "single'quote",
        "double\"quote",
        "back\\slash",
        "control\tcharacter",
        "line one\nline two\n",
    ];
    let mapping: BTreeMap<_, _> = strings.iter().map(|text| (*text, *text)).collect();
    for (old, migrated) in configurations() {
        for text in &strings {
            assert_same_output(text, &old, &migrated);
        }
        assert_same_output(&strings.as_slice(), &old, &migrated);
        assert_same_output(&FlowSeq(strings.as_slice()), &old, &migrated);
        assert_same_output(&mapping, &old, &migrated);
        assert_same_output(&FlowMap(&mapping), &old, &migrated);
    }
}

#[test]
fn legacy_and_specific_match_for_automatic_and_explicit_string_styles() {
    let strings = [
        "yes",
        "inf",
        "2001-12-15",
        "a long ordinary string with enough words to fold across multiple lines",
        "line one\nline two\n",
        "line one\nline two\n\n",
    ];
    for (mut old, mut migrated) in configurations() {
        for prefer_block_scalars in [false, true] {
            for options in [&mut old, &mut migrated] {
                options.prefer_block_scalars = prefer_block_scalars;
                options.min_fold_chars = 8;
                options.folded_wrap_chars = 24;
            }
            for text in &strings {
                assert_same_output(text, &old, &migrated);
                assert_same_output(&LitStr(text), &old, &migrated);
                assert_same_output(&FoldStr(text), &old, &migrated);
                assert_same_output(&DoubleQuoted(text), &old, &migrated);
                // Explicit single-quoted strings cannot contain line breaks.
                if !text.contains('\n') {
                    assert_same_output(&SingleQuoted(text), &old, &migrated);
                }
            }
        }
    }
}

#[test]
fn legacy_and_specific_match_for_typed_values_and_document_directives() {
    #[derive(Serialize)]
    struct Scalars {
        boolean: bool,
        integer: i64,
        float: f64,
        null: Option<u8>,
        text: &'static str,
    }
    let values = [
        Scalars {
            boolean: true,
            integer: -42,
            float: 1.25,
            null: None,
            text: "yes",
        },
        Scalars {
            boolean: false,
            integer: 8,
            float: f64::INFINITY,
            null: Some(1),
            text: "2001-12-15",
        },
    ];
    for (mut old, mut migrated) in configurations() {
        for no_lang_directive in [false, true] {
            old.no_lang_directive = no_lang_directive;
            migrated.no_lang_directive = no_lang_directive;
            for value in &values {
                assert_same_output(value, &old, &migrated);
            }
            for documents in [&values[..0], &values[..1], &values[..]] {
                let actual = to_string_multiple_with_options(documents, migrated.clone()).unwrap();
                assert_eq!(
                    actual,
                    to_string_multiple_with_options(documents, old.clone()).unwrap(),
                    "migrated options: {migrated:?}"
                );
                let expected_directives = if old.yaml_12 && !no_lang_directive {
                    documents.len()
                } else {
                    0
                };
                assert_eq!(actual.matches("%YAML 1.2\n").count(), expected_directives);
            }
        }
    }
}

#[test]
fn specific_serializer_flags_override_deprecated_options_in_both_directions() {
    let mapping = BTreeMap::from([("yes", "yes"), ("2001-12-15", "inf"), ("word", "word")]);
    for (old, migrated) in configurations() {
        for options in [
            ser_options! {
                schema: migrated.schema,
                yaml_12: !old.yaml_12,
                quote_all: !old.quote_all,
            },
            ser_options! {
                yaml_12: !old.yaml_12,
                quote_all: !old.quote_all,
                schema: migrated.schema,
            },
        ] {
            assert_same_output(&mapping, &old, &options);
        }
    }
}
