use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_saphyr as yaml;

#[derive(Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
enum Example {
    A = 1,
    B = 2,
}

#[test]
// This test requires serde_repr crate that is in development dependencies.
fn test_serde_repr_enum() {
    let value = Example::B;
    let yaml_str = "2\n";
    let serialized = yaml::to_string(&value).unwrap();
    assert_eq!(yaml_str, serialized);
    let deserialized: Example = yaml::from_str(yaml_str).unwrap();
    assert_eq!(value, deserialized);
}
