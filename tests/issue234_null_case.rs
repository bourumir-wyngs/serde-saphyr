#![cfg(feature = "deserialize")]

use serde_json::Value;
use std::collections::BTreeMap;

const MIXED_CASE_NULL_STRINGS: [&str; 13] = [
    "nulL", "nuLl", "nuLL", "nUll", "nUlL", "nULl", "nULL", "NulL", "NuLl", "NuLL", "NUll", "NUlL",
    "NULl",
];

#[test]
fn nonstandard_null_spellings_remain_strings() -> Result<(), serde_saphyr::Error> {
    for text in MIXED_CASE_NULL_STRINGS {
        let yaml = format!("--- {text}\n...\n");

        assert_eq!(serde_saphyr::from_str::<String>(&yaml)?, text);
        assert_eq!(serde_saphyr::from_str::<&str>(&yaml)?, text);
        assert_eq!(
            serde_saphyr::from_str::<Option<String>>(&yaml)?,
            Some(text.to_owned())
        );
        assert_eq!(
            serde_saphyr::from_str::<Value>(&yaml)?,
            Value::String(text.to_owned())
        );

        let options = serde_saphyr::options! { no_schema: true };
        assert_eq!(
            serde_saphyr::from_str_with_options::<String>(&yaml, options)?,
            text
        );
    }

    Ok(())
}

#[test]
fn standard_null_spellings_remain_null() -> Result<(), serde_saphyr::Error> {
    for text in ["", "~", "null", "Null", "NULL"] {
        let yaml = format!("--- {text}\n...\n");

        assert_eq!(
            serde_saphyr::from_str::<Option<String>>(&yaml)?,
            None,
            "{text:?}"
        );
        assert_eq!(
            serde_saphyr::from_str::<Value>(&yaml)?,
            Value::Null,
            "{text:?}"
        );
        assert!(serde_saphyr::from_str::<String>(&yaml).is_err(), "{text:?}");
    }

    Ok(())
}

#[test]
fn nonstandard_null_spellings_do_not_become_empty_values() {
    for text in MIXED_CASE_NULL_STRINGS {
        assert!(serde_saphyr::from_str::<()>(text).is_err(), "{text:?}");
        assert!(serde_saphyr::from_str::<Vec<String>>(text).is_err(), "{text:?}");
        assert!(
            serde_saphyr::from_str::<BTreeMap<String, String>>(text).is_err(),
            "{text:?}"
        );
    }
}

#[test]
fn nonstandard_null_documents_are_retained() -> Result<(), serde_saphyr::Error> {
    let yaml = "--- NulL\n--- kept\n";
    let expected = vec!["NulL".to_owned(), "kept".to_owned()];

    let multiple: Vec<String> = serde_saphyr::from_str_multiple(yaml)?;
    let mut reader = yaml.as_bytes();
    let streamed: Vec<String> =
        serde_saphyr::read::<_, String>(&mut reader).collect::<Result<_, _>>()?;

    assert_eq!(multiple, expected);
    assert_eq!(streamed, expected);

    Ok(())
}

#[test]
fn nonstandard_null_strings_are_invalid_merge_sources() {
    let yaml = "<<: NulL\nkept: value\n";
    let error = serde_saphyr::from_str::<BTreeMap<String, String>>(yaml).unwrap_err();

    assert!(matches!(
        error.without_snippet(),
        serde_saphyr::Error::MergeValueNotMapOrSeqOfMaps { .. }
    ));
}
