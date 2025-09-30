#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_saphyr::sf_serde::{DuplicateKeyPolicy, Error};
    use serde_saphyr::{
        Options, from_multiple, from_multiple_with_options, from_str, from_str_with_options,
    };
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Details {
        city: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Person {
        name: String,
        age: usize,
        details: Details,
    }

    #[test]
    fn simple_nested_struct() {
        let y = "name: John\nage: 80\ndetails: { city: Paris }\n";
        let p: Person = from_str(y).unwrap();
        assert_eq!(p.name, "John");
        assert_eq!(p.age, 80);
        assert_eq!(p.details.city, "Paris");
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Named {
        name: String,
    }

    #[test]
    fn anchors_and_aliases_map_clone() {
        let y = "a: &A { name: John }\nb: *A\n";
        let m: HashMap<String, Named> = from_str(y).unwrap();
        assert_eq!(m.get("a").unwrap().name, "John");
        assert_eq!(m.get("b").unwrap().name, "John");
    }

    #[test]
    fn multiple_documents_deserialize_into_vec() {
        let yaml = "---\nname: John\nage: 80\ndetails:\n  city: Paris\n---\nname: Jane\nage: 42\ndetails:\n  city: London\n";
        let people: Vec<Person> = from_multiple(yaml).unwrap();
        assert_eq!(people.len(), 2);
        assert_eq!(people[0].name, "John");
        assert_eq!(people[1].name, "Jane");
    }

    #[test]
    fn budget_violation_is_reported() {
        use std::collections::HashMap;

        let mut options = Options::default();
        if let Some(ref mut budget) = options.budget {
            budget.max_nodes = 1; // force a tiny budget to trigger the error
        }

        let yaml = "a: 1\n";
        let err = from_str_with_options::<HashMap<String, String>>(yaml, options).unwrap_err();
        assert!(matches!(err, Error::Message { msg, .. } if msg.contains("budget")));
    }

    #[test]
    fn multiple_documents_budget_violation() {
        use std::collections::HashMap;

        let mut options = Options::default();
        if let Some(ref mut budget) = options.budget {
            budget.max_nodes = 1; // ensure the budget error triggers
        }

        let yaml = "a: 1\n---\nb: 2\n";
        let err = from_multiple_with_options::<HashMap<String, String>>(yaml, options).unwrap_err();
        assert!(matches!(err, Error::Message { msg, .. } if msg.contains("budget")));
    }

    #[test]
    fn anchor_with_nested_nonanchored_container_records_balanced_events() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Inner {
            m: String,
        }
        #[derive(Deserialize, Debug, PartialEq)]
        struct Outer {
            k: Inner,
        }

        let y = "a: &A { k: { m: v } }\nb: *A\n";
        let m: HashMap<String, Outer> = from_str(y).unwrap();
        assert_eq!(m["a"].k.m, "v");
        assert_eq!(m["b"].k.m, "v");
    }

    // ---------- YAML 1.2 floats ----------
    #[derive(Debug, Deserialize)]
    struct Floats {
        a: f64,
        b: f64,
        c: f32,
    }

    #[test]
    fn yaml12_float_specials() {
        let y = "a: .nan\nb: +.inf\nc: -.inf\n";
        let v: Floats = from_str(y).unwrap();
        assert!(v.a.is_nan());
        assert!(v.b.is_infinite() && v.b.is_sign_positive());
        assert!(v.c.is_infinite() && (v.c.is_sign_negative() as bool));
    }

    // ---------- Duplicate key policy ----------
    #[test]
    fn duplicate_keys_error_policy() {
        let y = "a: 1\na: 2\n";
        let err = from_str::<HashMap<String, i32>>(y).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("duplicate mapping key: a"));
    }

    #[test]
    fn quoted_and_plain_null_keys_are_distinct() {
        let y = "\"null\": 1\nnull: 2\n";
        let map = from_str::<HashMap<String, i32>>(y).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("null"), Some(&2));
    }

    #[test]
    fn duplicate_keys_first_wins_policy() {
        let y = "a: 1\na: 2\nb: 3\n";
        let mut opt = Options::default();
        opt.duplicate_keys = DuplicateKeyPolicy::FirstWins;
        let m = from_str_with_options::<HashMap<String, i32>>(y, opt).unwrap();
        assert_eq!(m.get("a"), Some(&1));
        assert_eq!(m.get("b"), Some(&3));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn duplicate_sequence_keys_policies() {
        let y = "?\n  - 1\n  - 2\n: first\n?\n  - 1\n  - 2\n: second\n";

        // Error policy should reject duplicate sequence keys.
        let err = from_str::<HashMap<Vec<i32>, String>>(y).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("duplicate mapping key"));

        let mut opt = Options::default();
        opt.duplicate_keys = DuplicateKeyPolicy::FirstWins;
        let first = from_str_with_options::<HashMap<Vec<i32>, String>>(y, opt.clone()).unwrap();
        assert_eq!(first.get(&vec![1, 2]).map(String::as_str), Some("first"));
        assert_eq!(first.len(), 1);

        opt.duplicate_keys = DuplicateKeyPolicy::LastWins;
        let last = from_str_with_options::<HashMap<Vec<i32>, String>>(y, opt).unwrap();
        assert_eq!(last.get(&vec![1, 2]).map(String::as_str), Some("second"));
        assert_eq!(last.len(), 1);
    }

    #[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
    struct StructKey {
        a: i32,
        b: String,
    }

    #[test]
    fn duplicate_struct_keys_policies() {
        let y = "?\n  a: 1\n  b: foo\n: 7\n?\n  a: 1\n  b: foo\n: 9\n";

        let err = from_str::<HashMap<StructKey, i32>>(y).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("duplicate mapping key"));

        let mut opt = Options::default();
        opt.duplicate_keys = DuplicateKeyPolicy::FirstWins;
        let first = from_str_with_options::<HashMap<StructKey, i32>>(y, opt.clone()).unwrap();
        assert_eq!(
            first.get(&StructKey {
                a: 1,
                b: "foo".into()
            }),
            Some(&7)
        );
        assert_eq!(first.len(), 1);

        opt.duplicate_keys = DuplicateKeyPolicy::LastWins;
        let last = from_str_with_options::<HashMap<StructKey, i32>>(y, opt).unwrap();
        assert_eq!(
            last.get(&StructKey {
                a: 1,
                b: "foo".into()
            }),
            Some(&9)
        );
        assert_eq!(last.len(), 1);
    }

    #[cfg(test)]
    mod hardening_policy_fixed_yaml_tests {
        use super::*;
        use serde::Deserialize;
        use serde_saphyr::{Options, from_str_with_options};
        use std::collections::HashMap;

        // ---------- Duplicate key policy: LastWins ----------
        #[test]
        fn duplicate_keys_last_wins_policy() {
            let y = "a: 1\na: 2\nb: 3\n";
            let mut opt = Options::default();
            opt.duplicate_keys = DuplicateKeyPolicy::LastWins;
            let m = from_str_with_options::<HashMap<String, i32>>(y, opt).unwrap();
            assert_eq!(m.get("a"), Some(&2));
            assert_eq!(m.get("b"), Some(&3));
            assert_eq!(m.len(), 2);
        }

        // ---------- Alias-bomb hardening: per-anchor expansion cap ----------
        //
        // NOTE: The previous version placed the anchored node as a *separate root node*,
        // which is invalid when deserializing into a mapping. Here we anchor the value
        // under a key ("defs") so the whole document is a single mapping.
        #[test]
        fn alias_per_anchor_expansion_limit() {
            // Anchor &A once, then reference it three times; cap expansions at 2.
            let y = "defs: &A { k: v }\nx: *A\ny: *A\nz: *A\n";
            let mut opt = Options::default();
            opt.alias_limits.max_alias_expansions_per_anchor = 2;
            let err = from_str_with_options::<HashMap<String, HashMap<String, String>>>(y, opt)
                .unwrap_err();
            let msg = format!("{err}");
            assert!(
                msg.contains("alias expansion limit exceeded"),
                "unexpected error: {msg}"
            );
        }

        // ---------- Alias-bomb hardening: total replayed events cap ----------
        //
        // We define the anchor under "defs" and then use it twice in "list".
        // Each expansion of [1,2,3,4] injects 6 events (start, 4 scalars, end) → 12 total.
        // With a cap of 10 this must fail.
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Data {
            defs: Vec<u32>,
            list: Vec<Vec<u32>>,
        }

        #[test]
        fn alias_total_replayed_events_limit() {
            let y = "defs: &A [1, 2, 3, 4]\nlist: [*A, *A]\n";
            let mut opt = Options::default();
            opt.alias_limits.max_total_replayed_events = 10;
            let err = from_str_with_options::<Data>(y, opt).unwrap_err();
            let msg = format!("{err}");
            assert!(
                msg.contains("alias replay limit exceeded"),
                "unexpected error: {msg}"
            );
        }

        // ---------- Alias-bomb hardening: replay stack depth cap ----------
        //
        // Due to the design that *resolves aliases during recording* of anchors,
        // nested alias chains are flattened before use. To still verify the guard,
        // we set the maximum replay stack depth to 0 and trigger a single alias,
        // which must fail immediately.
        #[test]
        fn alias_replay_stack_depth_limit() {
            let y = "defs: &A [1]\nout: *A\n";
            let mut opt = Options::default();
            opt.alias_limits.max_replay_stack_depth = 0; // any alias use should exceed this
            let err = from_str_with_options::<HashMap<String, Vec<u32>>>(y, opt).unwrap_err();
            let msg = format!("{err}");
            assert!(
                msg.contains("alias replay stack depth exceeded"),
                "unexpected error: {msg}"
            );
        }

        // Place the anchor *inside* the mapping so the document root is a mapping
        // (which matches HashMap<_, _>), then alias it multiple times to exceed the budget.
        #[test]
        fn alias_replay_counts_toward_budget() {
            let mut options = Options::default();
            if let Some(ref mut b) = options.budget {
                b.max_nodes = 10;
            }

            // Root is a mapping with key "seq". First element defines &A, the rest alias it.
            let y = "\
                seq:
                  - &A [1,2,3]
                  - *A
                  - *A
                  - *A
                  - *A
                ";
            let err =
                from_str_with_options::<HashMap<String, Vec<Vec<u32>>>>(y, options).unwrap_err();
            assert!(format!("{err}").contains("budget"));
        }
    }
}
