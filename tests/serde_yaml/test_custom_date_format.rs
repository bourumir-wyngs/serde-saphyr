use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

time::serde::format_description!(
    date_format,
    PrimitiveDateTime,
    "[year]-[month]-[day]! [hour]:[minute]:[second]"
);

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Record {
    #[serde(with = "date_format")]
    timestamp: PrimitiveDateTime,
    tester: String,
}

#[test]
fn custom_date_format_round_trips() {
    let original = Record {
        timestamp: time::macros::datetime!(2025-07-25 11:32:42),
        tester: "Bourumir".to_owned(),
    };

    let serialized = serde_saphyr::to_string(&original).unwrap();
    assert_eq!(
        serialized,
        "timestamp: 2025-07-25! 11:32:42\ntester: Bourumir\n"
    );

    let deserialized = serde_saphyr::from_str(&serialized).unwrap();
    assert_eq!(original, deserialized);
}
