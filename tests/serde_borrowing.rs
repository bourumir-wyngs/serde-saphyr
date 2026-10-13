#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;
use serde::de::value::BorrowedStrDeserializer;
use std::borrow::Cow;

#[derive(Deserialize)]
#[serde(transparent)]
struct Borrowing<'a> {
    // Serde defaults Cow<str> to Cow::Owned. Opting into borrowing adds a
    // generated 'de: 'a bound, so borrowed input must outlive the result.
    // Values requiring escape decoding can still fall back to Cow::Owned.
    // See https://serde.rs/lifetimes.html#borrowing-data-in-a-derived-impl
    #[serde(borrow)]
    value: Cow<'a, str>,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct NoBorrowing<'a> {
    // The cow will be owned
    value: Cow<'a, str>,
}

#[test]
fn cow_str_borrows_only_when_explicitly_enabled() {
    let input = String::from("hello");
    let de = BorrowedStrDeserializer::<serde::de::value::Error>::new(input.as_str());

    let value: Borrowing<'_> = Deserialize::deserialize(de).unwrap();

    match value.value {
        Cow::Borrowed(s) => {
            assert_eq!(s, input);
            assert!(std::ptr::eq(s, input.as_str()));
        }
        Cow::Owned(s) => panic!("expected borrowed Cow, got owned: {s}"),
    }

    let de = BorrowedStrDeserializer::<serde::de::value::Error>::new(input.as_str());
    let value: NoBorrowing<'_> = Deserialize::deserialize(de).unwrap();

    assert_eq!(value.value, input);
    assert!(matches!(value.value, Cow::Owned(_)));
}

#[test]
fn cow_str_owns_escaped_yaml_with_or_without_borrowing() {
    // Decoding the escape creates a newline that is absent from the input.
    let input = r#""hello\nworld""#;
    let escaped = "hello\nworld";

    let borrowing: Borrowing<'_> = serde_saphyr::from_str(input).unwrap();
    assert_eq!(borrowing.value, escaped);
    assert!(matches!(borrowing.value, Cow::Owned(_)));

    let no_borrowing: NoBorrowing<'_> = serde_saphyr::from_str(input).unwrap();
    assert_eq!(no_borrowing.value, escaped);
    assert!(matches!(no_borrowing.value, Cow::Owned(_)));
}
