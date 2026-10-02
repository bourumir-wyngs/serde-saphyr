#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;
use std::fmt::Debug;

use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::{Budget, DuplicateKeyPolicy, Error, MergeKeyPolicy, Options};

const RECURSIVE_MERGE: &str = "a: &a\n  b:\n    <<: *a\n";

fn assert_recursive_error<T: Debug>(result: Result<T, Error>) {
    let error = result.expect_err("recursive merge aliases must return an error");
    match error.without_snippet() {
        Error::RecursiveReferencesRequireWeakTypes { .. } => {}
        // Buffered values wrap the error with the alias locations.
        Error::AliasError { error: inner, .. } => assert!(
            matches!(
                inner.as_ref(),
                Error::RecursiveReferencesRequireWeakTypes { .. }
            ),
            "{error:?}"
        ),
        _ => panic!("unexpected error: {error:?}"),
    }
}

#[test]
fn recursive_merge_is_rejected_across_input_apis() {
    assert_recursive_error(serde_saphyr::from_str::<Value>(RECURSIVE_MERGE));
    assert_recursive_error(serde_saphyr::from_slice::<Value>(
        RECURSIVE_MERGE.as_bytes(),
    ));
    assert_recursive_error(serde_saphyr::from_reader::<_, Value>(
        RECURSIVE_MERGE.as_bytes(),
    ));
    assert_recursive_error(serde_saphyr::with_deserializer_from_str(
        RECURSIVE_MERGE,
        |de| Value::deserialize(de),
    ));
    assert_recursive_error(serde_saphyr::from_str_multiple::<Value, Vec<Value>>(
        RECURSIVE_MERGE,
    ));
    assert_recursive_error(serde_saphyr::from_bytes_multiple::<Value, Vec<Value>>(
        RECURSIVE_MERGE.as_bytes(),
    ));

    let error = serde_saphyr::from_str::<Value>(RECURSIVE_MERGE).unwrap_err();
    let location = error.location().expect("recursive alias location");
    assert_eq!((location.line(), location.column()), (3, 9));
}

#[test]
fn recursive_merge_variants_do_not_depend_on_resource_limits() {
    let documents = [
        RECURSIVE_MERGE,
        "{a: &a {b: {<<: *a}}}\n",
        "a: &a\n  b:\n    <<: [*a]\n",
        "a: &a\n  b: &b\n    <<: *a\n",
        "a: &a\n  b:\n    !!merge '<<': *a\n",
        "a: &a\n  b:\n    !<tag:yaml.org,2002:merge> '<<': *a\n",
        "%TAG !m! tag:yaml.org,2002:\n---\na: &a\n  b:\n    !m!merge '<<': *a\n",
    ];

    for yaml in documents {
        for duplicate_keys in [
            DuplicateKeyPolicy::Error,
            DuplicateKeyPolicy::FirstWins,
            DuplicateKeyPolicy::LastWins,
        ] {
            for budget in [Some(Budget::default()), None] {
                // The recursion guard must work even with every resource cap disabled.
                let options = serde_saphyr::options! {
                    duplicate_keys: duplicate_keys,
                    budget: budget,
                    alias_limits: serde_saphyr::alias_limits! {
                        max_total_replayed_events: usize::MAX,
                        max_replay_stack_depth: usize::MAX,
                        max_alias_expansions_per_anchor: usize::MAX,
                    },
                };
                assert_recursive_error(serde_saphyr::from_str_with_options::<Value>(
                    yaml,
                    options.clone(),
                ));
                assert_recursive_error(serde_saphyr::from_reader_with_options::<_, Value>(
                    yaml.as_bytes(),
                    options,
                ));
            }
        }
    }
}

#[test]
fn recursive_merge_is_rejected_after_last_wins_buffering() {
    for yaml in [
        "1: &a\n  2:\n    <<: *a\n",
        "1: &a\n  2:\n    <<: [*a]\n",
        // The first value is discarded; the completed anchor is merged later.
        "1: &a { b: { <<: *a } }\n1: {}\n2: { <<: *a }\n",
    ] {
        let options = serde_saphyr::options! {
            duplicate_keys: DuplicateKeyPolicy::LastWins,
            budget: None,
        };
        assert_recursive_error(serde_saphyr::from_str_with_options::<BTreeMap<u32, Value>>(
            yaml,
            options.clone(),
        ));
        assert_recursive_error(serde_saphyr::from_reader_with_options::<
            _,
            BTreeMap<u32, Value>,
        >(yaml.as_bytes(), options));
    }
}

#[test]
fn recursive_merge_is_safe_under_each_merge_key_policy() {
    for merge_keys in [MergeKeyPolicy::Merge, MergeKeyPolicy::AsOrdinary] {
        let options = serde_saphyr::options! {
            merge_keys: merge_keys,
            budget: None,
        };
        assert_recursive_error(serde_saphyr::from_str_with_options::<Value>(
            RECURSIVE_MERGE,
            options,
        ));
    }

    let options = serde_saphyr::options! {
        merge_keys: MergeKeyPolicy::Error,
    };
    let error = serde_saphyr::from_str_with_options::<Value>(RECURSIVE_MERGE, options)
        .expect_err("merge keys must be rejected before expansion");
    assert!(matches!(
        error.without_snippet(),
        Error::MergeKeyNotAllowed { .. }
    ));
}

#[test]
fn reader_recovers_after_recursive_merge() {
    let yaml = format!("{RECURSIVE_MERGE}---\nok: true\n");
    let mut reader = yaml.as_bytes();
    let mut documents =
        serde_saphyr::read_with_options::<_, Value>(&mut reader, Options::default());
    assert_recursive_error(documents.next().expect("first document"));
    assert_eq!(documents.next().unwrap().unwrap(), json!({"ok": true}));
    assert!(documents.next().is_none());
}

#[test]
fn non_recursive_nested_merge_still_expands() {
    let yaml = "base: &base { value: 42 }\na:\n  b:\n    <<: *base\n";
    let expected = json!({"base": {"value": 42}, "a": {"b": {"value": 42}}});
    assert_eq!(serde_saphyr::from_str::<Value>(yaml).unwrap(), expected);
    assert_eq!(
        serde_saphyr::from_reader::<_, Value>(yaml.as_bytes()).unwrap(),
        expected
    );
}
