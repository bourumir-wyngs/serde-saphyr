#![cfg(feature = "serialize")]

// Regression coverage for https://github.com/dtolnay/serde-yaml/issues/348.
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
struct Glyph {
    name: String,
    sources: BTreeMap<BTreeMap<String, i32>, PathBuf>,
}

#[test]
fn singleton_map_serializes_as_key_of_singleton_map() {
    let glyph = Glyph {
        name: "foo".to_owned(),
        sources: BTreeMap::from([(
            BTreeMap::from([("Weight".to_owned(), 400)]),
            PathBuf::from("Font-Regular.ufo/glyphs/foo.glif"),
        )]),
    };

    let yaml = serde_saphyr::to_string(&glyph)
        .expect("a singleton map should serialize as a complex mapping key");
    assert_eq!(
        yaml,
        "name: foo\nsources:\n  ? Weight: 400\n  : Font-Regular.ufo/glyphs/foo.glif\n"
    );

    #[cfg(feature = "deserialize")]
    {
        let round_trip: Glyph = serde_saphyr::from_str(&yaml)
            .expect("the emitted complex mapping key should deserialize");
        assert_eq!(round_trip, glyph);
    }
}
