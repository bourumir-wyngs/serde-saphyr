use serde::de::Deserialize;
use serde::de::value::BorrowedStrDeserializer;
use std::borrow::Cow;

#[test]
#[ignore]
fn cow_str_should_borrow_when_deserializer_offers_borrowed_str() {
    let input = "hello";
    let de = BorrowedStrDeserializer::<serde::de::value::Error>::new(input);

    let value: Cow<'_, str> = Deserialize::deserialize(de).unwrap();

    // Expected: Cow::Borrowed("hello")
    // Actual:   Cow::Owned("hello".to_string())
    match value {
        Cow::Borrowed(_) => {}
        Cow::Owned(s) => panic!("expected borrowed Cow, got owned: {s}"),
    }
}