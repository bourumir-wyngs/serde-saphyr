#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_json::Value;
use serde_saphyr::Error;
use std::collections::BTreeMap;

fn strict<'de, T: Deserialize<'de>>(yaml: &'de str, no_schema: bool) -> Result<T, Error> {
    serde_saphyr::from_str_with_options(
        yaml,
        serde_saphyr::options! {
            strict_booleans: true,
            no_schema: no_schema,
        },
    )
}

fn case_variants(word: &str) -> impl Iterator<Item = (String, bool)> + '_ {
    let count = 1usize << word.len();

    (0..count).map(move |mask| {
        let text = word
            .bytes()
            .enumerate()
            .map(|(index, byte)| {
                char::from(if mask & (1usize << index) == 0 {
                    byte
                } else {
                    byte.to_ascii_uppercase()
                })
            })
            .collect();

        // Core schema allows lowercase, title case, and uppercase.
        let accepted = mask == 0 || mask == 1 || mask == count - 1;
        (text, accepted)
    })
}

#[rstest]
#[case::true_word("true", true)]
#[case::false_word("false", false)]
fn strict_boolean_capitalization_is_consistent_across_targets(
    #[case] word: &str,
    #[case] expected: bool,
) -> Result<(), Error> {
    for (text, accepted) in case_variants(word) {
        // Typed conversion retains its existing quoted/padded behavior.
        for yaml in [
            text.clone(),
            format!("\" \t{text}\t \""),
            format!("!!bool {text}"),
            format!("!!bool \" \t{text}\t \""),
        ] {
            let result = strict::<bool>(&yaml, false);
            if accepted {
                assert_eq!(result?, expected, "{yaml}");
            } else {
                let error = result.unwrap_err();
                assert!(
                    matches!(error.without_snippet(), Error::InvalidBooleanStrict { .. }),
                    "{yaml}: {error:?}"
                );
            }
        }

        let inferred = if accepted {
            Value::Bool(expected)
        } else {
            Value::String(text.clone())
        };
        assert_eq!(strict::<Value>(&text, false)?, inferred, "{text}");

        // Explicit bool tags validate their contents, even when quoted.
        for yaml in [
            format!("!!bool {text}"),
            format!("!!bool \" \t{text}\t \""),
        ] {
            let result = strict::<Value>(&yaml, false);
            if accepted {
                assert_eq!(result?, Value::Bool(expected), "{yaml}");
            } else {
                let error = result.unwrap_err();
                assert!(
                    matches!(error.without_snippet(), Error::InvalidBooleanStrict { .. }),
                    "{yaml}: {error:?}"
                );
            }
        }
    }

    Ok(())
}

#[rstest]
fn strict_boolean_capitalization_controls_string_quoting(
    #[values("true", "false")] word: &str,
) -> Result<(), Error> {
    for (text, accepted) in case_variants(word) {
        if accepted {
            let error = strict::<String>(&text, true).unwrap_err();
            assert!(
                matches!(error.without_snippet(), Error::QuotingRequired { .. }),
                "{text}: {error:?}"
            );
        } else {
            assert_eq!(strict::<String>(&text, true)?, text);
            assert_eq!(strict::<&str>(&text, true)?, text);

            let yaml = format!("{text}: 1\n");
            let map = strict::<BTreeMap<String, u8>>(&yaml, true)?;
            assert_eq!(map, BTreeMap::from([(text.clone(), 1)]));
        }
    }

    Ok(())
}

/// Preserve `text` when `prefix` and `suffix` wrap it in YAML string syntax.
/// Returns `Ok(())` when both deserialization paths preserve the scalar text.
#[rstest]
#[case::single_quoted("'", "'")]
#[case::double_quoted("\"", "\"")]
#[case::string_tag("!!str ", "")]
#[case::non_specific_tag("! ", "")]
fn strict_booleans_preserve_quoted_and_tagged_strings(
    #[values("true", "True", "TRUE", "false", "False", "FALSE", "tRuE", "fAlSe")] text: &str,
    #[case] prefix: &str,
    #[case] suffix: &str,
) -> Result<(), Error> {
    let yaml = format!("{prefix}{text}{suffix}");
    assert_eq!(strict::<String>(&yaml, true)?, text);
    assert_eq!(
        strict::<Value>(&yaml, false)?,
        Value::String(text.to_owned()),
        "{yaml}"
    );

    Ok(())
}

#[rstest]
#[case::true_word("true", true)]
#[case::false_word("false", false)]
fn non_strict_booleans_keep_case_insensitive_matching(
    #[case] word: &str,
    #[case] expected: bool,
) -> Result<(), Error> {
    for (text, _) in case_variants(word) {
        assert_eq!(serde_saphyr::from_str::<bool>(&text)?, expected, "{text}");
        assert_eq!(
            serde_saphyr::from_str::<Value>(&text)?,
            Value::Bool(expected),
            "{text}"
        );
    }

    Ok(())
}
