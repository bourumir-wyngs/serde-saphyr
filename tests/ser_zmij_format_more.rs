use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;
use serde_saphyr::to_string;

struct MapWithFloatKeys;

impl Serialize for MapWithFloatKeys {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry(&f32::NAN, &1)?;
        map.serialize_entry(&f32::INFINITY, &2)?;
        map.serialize_entry(&f32::NEG_INFINITY, &3)?;
        map.serialize_entry(&1e20f32, &4)?;
        map.end()
    }
}

#[test]
fn test_float_map_keys_nan_inf() {
    let out = to_string(&MapWithFloatKeys).unwrap();
    assert!(out.contains(".nan: 1"));
    assert!(out.contains(".inf: 2"));
    assert!(out.contains("-.inf: 3"));
    assert!(out.contains("1.0e+20: 4"));
}
