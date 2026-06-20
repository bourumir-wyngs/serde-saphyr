#![cfg(any(feature = "serialize", feature = "deserialize"))]

#[cfg(feature = "serialize")]
mod serialize_tests {
    use serde::Serialize;
    use serde::ser::{self, SerializeMap, Serializer};
    use serde_saphyr::{
        Commented, DoubleQuoted, FlowMap, FlowSeq, NullableTilde, RcAnchor, SpaceAfter, to_string,
    };
    use std::collections::BTreeMap;
    use std::fmt;
    use std::rc::Rc;

    #[derive(Debug, PartialEq)]
    enum Event {
        None,
        Some(Box<Event>),
        Newtype(&'static str, Box<Event>),
        Unit,
        Bool(bool),
        I64(i64),
        U64(u64),
        Str(String),
    }

    #[derive(Debug)]
    struct RecordingError(String);

    impl fmt::Display for RecordingError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.0)
        }
    }

    impl std::error::Error for RecordingError {}

    impl ser::Error for RecordingError {
        fn custom<T: fmt::Display>(message: T) -> Self {
            Self(message.to_string())
        }
    }

    fn unsupported<T>() -> Result<T, RecordingError> {
        Err(ser::Error::custom("unsupported"))
    }

    struct NonHumanReadableSerializer;

    impl Serializer for NonHumanReadableSerializer {
        type Ok = Event;
        type Error = RecordingError;

        type SerializeSeq = ser::Impossible<Event, RecordingError>;
        type SerializeTuple = ser::Impossible<Event, RecordingError>;
        type SerializeTupleStruct = ser::Impossible<Event, RecordingError>;
        type SerializeTupleVariant = ser::Impossible<Event, RecordingError>;
        type SerializeMap = ser::Impossible<Event, RecordingError>;
        type SerializeStruct = ser::Impossible<Event, RecordingError>;
        type SerializeStructVariant = ser::Impossible<Event, RecordingError>;

        fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
            Ok(Event::Bool(value))
        }

        fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
            Ok(Event::I64(value.into()))
        }

        fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
            Ok(Event::I64(value.into()))
        }

        fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
            Ok(Event::I64(value.into()))
        }

        fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
            Ok(Event::I64(value))
        }

        fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
            Ok(Event::U64(value.into()))
        }

        fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
            Ok(Event::U64(value.into()))
        }

        fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
            Ok(Event::U64(value.into()))
        }

        fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
            Ok(Event::U64(value))
        }

        fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
            unsupported()
        }

        fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
            unsupported()
        }

        fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
            Ok(Event::Str(value.to_string()))
        }

        fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
            Ok(Event::Str(value.to_string()))
        }

        fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
            unsupported()
        }

        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            Ok(Event::None)
        }

        fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
            value
                .serialize(NonHumanReadableSerializer)
                .map(|event| Event::Some(Box::new(event)))
        }

        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
            Ok(Event::Unit)
        }

        fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
            Ok(Event::Unit)
        }

        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
        ) -> Result<Self::Ok, Self::Error> {
            Ok(Event::Str(variant.to_string()))
        }

        fn serialize_newtype_struct<T: ?Sized + Serialize>(
            self,
            name: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            value
                .serialize(NonHumanReadableSerializer)
                .map(|event| Event::Newtype(name, Box::new(event)))
        }

        fn serialize_newtype_variant<T: ?Sized + Serialize>(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            value
                .serialize(NonHumanReadableSerializer)
                .map(|event| Event::Newtype(variant, Box::new(event)))
        }

        fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            unsupported()
        }

        fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
            unsupported()
        }

        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            unsupported()
        }

        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            unsupported()
        }

        fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            unsupported()
        }

        fn serialize_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            unsupported()
        }

        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            unsupported()
        }

        fn is_human_readable(&self) -> bool {
            false
        }
    }

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
    fn non_human_readable_serializers_receive_option_data_model() {
        let none = NullableTilde::<i32>(None)
            .serialize(NonHumanReadableSerializer)
            .unwrap();
        assert_eq!(none, Event::None);

        let some = NullableTilde(Some(7))
            .serialize(NonHumanReadableSerializer)
            .unwrap();
        assert_eq!(some, Event::Some(Box::new(Event::I64(7))));
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

    #[test]
    fn nullable_tilde_some_preserves_strong_anchor_serialization() {
        #[derive(Serialize)]
        struct Doc {
            first: NullableTilde<RcAnchor<String>>,
            second: NullableTilde<RcAnchor<String>>,
        }

        let shared = Rc::new("shared".to_string());
        let doc = Doc {
            first: NullableTilde(Some(RcAnchor(shared.clone()))),
            second: NullableTilde(Some(RcAnchor(shared))),
        };

        assert_eq!(to_string(&doc).unwrap(), "first: &a1 shared\nsecond: *a1\n");
    }

    #[test]
    fn strong_anchor_wrapping_nullable_tilde_none_attaches_anchor_to_tilde() {
        #[derive(Serialize)]
        struct Doc {
            first: RcAnchor<NullableTilde<i32>>,
            second: RcAnchor<NullableTilde<i32>>,
        }

        let shared = Rc::new(NullableTilde(None));
        let doc = Doc {
            first: RcAnchor(shared.clone()),
            second: RcAnchor(shared),
        };

        assert_eq!(to_string(&doc).unwrap(), "first: &a1 ~\nsecond: *a1\n");
    }

    #[test]
    fn flow_seq_wrapper_around_nullable_none_does_not_leak_flow_hint() {
        #[derive(Serialize)]
        struct Doc {
            maybe: FlowSeq<NullableTilde<Vec<i32>>>,
            next: Vec<i32>,
        }

        let doc = Doc {
            maybe: FlowSeq(NullableTilde(None)),
            next: vec![1, 2],
        };

        assert_eq!(to_string(&doc).unwrap(), "maybe: ~\nnext:\n- 1\n- 2\n");
    }

    #[test]
    fn flow_map_wrapper_around_nullable_none_does_not_leak_flow_hint() {
        #[derive(Serialize)]
        struct Doc {
            maybe: FlowMap<NullableTilde<BTreeMap<String, i32>>>,
            next: BTreeMap<String, i32>,
        }

        let mut next = BTreeMap::new();
        next.insert("a".to_string(), 1);

        let doc = Doc {
            maybe: FlowMap(NullableTilde(None)),
            next,
        };

        assert_eq!(to_string(&doc).unwrap(), "maybe: ~\nnext:\n  a: 1\n");
    }
}

#[cfg(feature = "deserialize")]
mod deserialize_tests {
    use serde::Deserialize;
    use serde_saphyr::{Commented, FlowSeq, NullableTilde, RcAnchor, from_str};
    use std::rc::Rc;

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

    #[test]
    fn nullable_tilde_some_deserializes_strong_anchor_identity() {
        #[derive(Debug, Deserialize)]
        struct Doc {
            first: NullableTilde<RcAnchor<String>>,
            second: NullableTilde<RcAnchor<String>>,
            absent: NullableTilde<RcAnchor<String>>,
        }

        let doc: Doc = from_str("first: &a shared\nsecond: *a\nabsent: ~\n").unwrap();
        let first = doc.first.0.as_ref().expect("first should be Some");
        let second = doc.second.0.as_ref().expect("second should be Some");

        assert_eq!(first.0.as_str(), "shared");
        assert!(Rc::ptr_eq(&first.0, &second.0));
        assert_eq!(doc.absent, NullableTilde(None));
    }

    #[test]
    fn strong_anchor_wrapping_nullable_tilde_none_deserializes_alias_identity() {
        #[derive(Debug, Deserialize)]
        struct Doc {
            first: RcAnchor<NullableTilde<String>>,
            second: RcAnchor<NullableTilde<String>>,
        }

        let doc: Doc = from_str("first: &a ~\nsecond: *a\n").unwrap();

        assert_eq!(doc.first.0.as_ref(), &NullableTilde(None));
        assert!(Rc::ptr_eq(&doc.first.0, &doc.second.0));
    }
}
