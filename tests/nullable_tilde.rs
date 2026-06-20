#![cfg(any(feature = "serialize", feature = "deserialize"))]

#[cfg(feature = "serialize")]
mod serialize_tests {
    use serde::Serialize;
    use serde::ser::{SerializeMap, Serializer};
    use serde_saphyr::{Commented, DoubleQuoted, FlowSeq, NullableTilde, SpaceAfter, to_string};

    #[test]
    fn serializes_none_as_tilde_without_changing_plain_option() {
        #[derive(Serialize)]
        struct Doc {
            tilde: NullableTilde<String>,
            present: NullableTilde<String>,
            ordinary_none: Option<String>,
        }

        let doc = Doc {
            tilde: NullableTilde(None),
            present: NullableTilde(Some("value".to_string())),
            ordinary_none: None,
        };

        assert_eq!(
            to_string(&doc).unwrap(),
            "tilde: ~\npresent: value\nordinary_none: null\n"
        );
    }

    #[test]
    fn serializes_top_level_none_as_tilde() {
        assert_eq!(to_string(&NullableTilde::<i32>(None)).unwrap(), "~\n");
    }

    #[test]
    fn combines_with_other_wrappers() {
        #[derive(Serialize)]
        struct Doc {
            commented: Commented<NullableTilde<i32>>,
            spaced: SpaceAfter<NullableTilde<i32>>,
            flow: FlowSeq<Vec<NullableTilde<i32>>>,
            flow_some: NullableTilde<FlowSeq<Vec<i32>>>,
            quoted_some: NullableTilde<DoubleQuoted<&'static str>>,
        }

        let doc = Doc {
            commented: Commented(NullableTilde(None), "absent".to_string()),
            spaced: SpaceAfter(NullableTilde(None)),
            flow: FlowSeq(vec![
                NullableTilde(Some(1)),
                NullableTilde(None),
                NullableTilde(Some(3)),
            ]),
            flow_some: NullableTilde(Some(FlowSeq(vec![4, 5]))),
            quoted_some: NullableTilde(Some(DoubleQuoted("plain"))),
        };

        assert_eq!(
            to_string(&doc).unwrap(),
            "commented: ~ # absent\nspaced: ~\n\nflow: [1, ~, 3]\nflow_some: [4, 5]\nquoted_some: \"plain\"\n"
        );
    }

    #[test]
    fn serializes_none_map_keys_as_tilde() {
        struct Keyed;

        impl Serialize for Keyed {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry(&NullableTilde::<String>(None), &1)?;
                map.serialize_entry(&NullableTilde(Some("present".to_string())), &2)?;
                map.end()
            }
        }

        assert_eq!(to_string(&Keyed).unwrap(), "~: 1\npresent: 2\n");
    }
}

#[cfg(feature = "deserialize")]
mod deserialize_tests {
    use serde::Deserialize;
    use serde_saphyr::{Commented, FlowSeq, NullableTilde, from_str};

    #[test]
    fn deserializes_like_option() {
        for yaml in ["~\n", "null\n", "\n", "value\n"] {
            let wrapped: NullableTilde<String> = from_str(yaml).unwrap();
            let option: Option<String> = from_str(yaml).unwrap();
            assert_eq!(wrapped.0, option, "yaml: {yaml:?}");
        }
    }

    #[test]
    fn deserializes_when_combined_with_other_wrappers() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Doc {
            tilde: NullableTilde<String>,
            explicit_null: NullableTilde<String>,
            empty: NullableTilde<String>,
            value: NullableTilde<String>,
            flow_some: NullableTilde<FlowSeq<Vec<i32>>>,
            commented: Commented<NullableTilde<bool>>,
        }

        let doc: Doc = from_str(
            "tilde: ~\nexplicit_null: null\nempty:\nvalue: hello\nflow_some: [1, 2]\ncommented: true\n",
        )
        .unwrap();

        assert_eq!(doc.tilde, NullableTilde(None));
        assert_eq!(doc.explicit_null, NullableTilde(None));
        assert_eq!(doc.empty, NullableTilde(None));
        assert_eq!(doc.value, NullableTilde(Some("hello".to_string())));
        assert_eq!(doc.flow_some, NullableTilde(Some(FlowSeq(vec![1, 2]))));
        assert_eq!(
            doc.commented,
            Commented(NullableTilde(Some(true)), String::new())
        );
    }
}
