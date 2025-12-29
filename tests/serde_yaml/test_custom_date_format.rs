use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

// This test requires chrono that is in development dependencies.

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct StructWithDate {
    #[serde(with = "some_date_format")] // "some_date_format" defined below
    pub timestamp: DateTime<Utc>,

    pub tester: String,
}

mod some_date_format {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&date.format(FORMAT).to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)?;
        Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
    }
}

#[test]
fn test_custom_date_serialization() {
    let original = StructWithDate {
        timestamp: Utc.with_ymd_and_hms(2025, 7, 25, 11, 32, 42).unwrap(),
        tester: "Bourumir".to_string(),
    };

    // Serialize struct to JSON
    let serialized = serde_saphyr::to_string(&original).expect("Serialization failed");
    assert_eq!(
        serialized,
        "timestamp: 2025-07-25 11:32:42\ntester: Bourumir\n"
    );

    // Deserialize back from JSON
    let deserialized: StructWithDate =
        serde_saphyr::from_str(&serialized).expect("Deserialization failed");

    // Assert equality
    assert_eq!(original, deserialized);
}
