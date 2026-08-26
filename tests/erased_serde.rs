#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

trait Bike: erased_serde::Serialize {
    fn name(&self) -> &str;
}

erased_serde::serialize_trait_object!(Bike);

#[derive(Serialize)]
struct Motorbike {
    engine_power: f32,
}

impl Bike for Motorbike {
    fn name(&self) -> &str {
        "motorbike"
    }
}

#[derive(Serialize)]
struct Bicycle {
    ebike: bool,
}

impl Bike for Bicycle {
    fn name(&self) -> &str {
        "bicycle"
    }
}

#[test]
fn boxed_trait_object_serializes() {
    let bike: Box<dyn Bike> = Box::new(Motorbike {
        engine_power: 600.0,
    });

    let yaml = serde_saphyr::to_string(&bike).expect("trait object serialization must succeed");

    assert_eq!(yaml, "engine_power: 600.0\n");
    assert_eq!(bike.name(), "motorbike");
}

#[derive(Serialize)]
struct BorrowedBike<'a> {
    bike: &'a dyn Bike,
}

#[test]
fn borrowed_trait_object_serializes_when_nested() {
    let bike = Bicycle { ebike: true };
    let borrowed = BorrowedBike { bike: &bike };

    let yaml = serde_saphyr::to_string(&borrowed).expect("borrowed bike must serialize");

    assert_eq!(yaml, "bike:\n  ebike: true\n");
}

#[derive(Serialize)]
struct Garage {
    bikes: Vec<Box<dyn Bike>>,
}

#[test]
fn heterogeneous_trait_object_collection_serializes() {
    let garage = Garage {
        bikes: vec![
            Box::new(Motorbike {
                engine_power: 125.0,
            }),
            Box::new(Bicycle { ebike: false }),
        ],
    };

    let yaml = serde_saphyr::to_string(&garage).expect("garage serialization must succeed");

    assert_eq!(yaml, "bikes:\n- engine_power: 125.0\n- ebike: false\n");
}

struct Owners(Vec<(Box<dyn Bike>, Vec<String>)>);

impl Serialize for Owners {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (bike, owners) in &self.0 {
            map.serialize_entry(bike, owners)?;
        }
        map.end()
    }
}

#[test]
fn trait_object_serializes_as_mapping_key() {
    let owners = Owners(vec![(
        Box::new(Bicycle { ebike: true }),
        vec!["Alice".to_owned(), "Bob".to_owned()],
    )]);

    let yaml = serde_saphyr::to_string(&owners).expect("owners serialization must succeed");

    assert_eq!(yaml, "? ebike: true\n:\n- Alice\n- Bob\n");
}

#[test]
fn heterogeneous_erased_values_serialize() {
    let values: Vec<Box<dyn erased_serde::Serialize>> = vec![
        Box::new(7_u32),
        Box::new("cargo"),
        Box::new(vec![true, false]),
    ];

    let yaml = serde_saphyr::to_string(&values).expect("erased values must serialize");

    assert_eq!(yaml, "- 7\n- cargo\n- - true\n  - false\n");
}

#[test]
fn serde_saphyr_serializer_can_be_type_erased() {
    let value: Box<dyn erased_serde::Serialize> = Box::new(Motorbike { engine_power: 50.0 });
    let mut output = String::new();

    {
        let mut serializer = serde_saphyr::Serializer::new(&mut output);
        let mut serializer = <dyn erased_serde::Serializer>::erase(&mut serializer);
        value
            .erased_serialize(&mut serializer)
            .expect("type-erased serializer must accept the value");
    }

    assert_eq!(output, "engine_power: 50.0\n");
}

fn deserialize_erased<'de, T>(yaml: &'de str) -> Result<T, serde_saphyr::Error>
where
    T: Deserialize<'de>,
{
    serde_saphyr::with_deserializer_from_str(yaml, |deserializer| {
        let mut deserializer = <dyn erased_serde::Deserializer>::erase(deserializer);
        erased_serde::deserialize(&mut deserializer)
            .map_err(<serde_saphyr::Error as serde::de::Error>::custom)
    })
}

#[derive(Debug, Deserialize, PartialEq)]
struct Workshop {
    name: String,
    open: bool,
    bikes: Vec<StoredBike>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct StoredBike {
    name: String,
    gears: u8,
}

#[test]
fn serde_saphyr_deserializer_can_be_type_erased() {
    let yaml = "name: Wheel Works\nopen: true\nbikes:\n- name: Commuter\n  gears: 8\n";

    let workshop: Workshop =
        deserialize_erased(yaml).expect("type-erased deserializer must produce a value");

    assert_eq!(
        workshop,
        Workshop {
            name: "Wheel Works".to_owned(),
            open: true,
            bikes: vec![StoredBike {
                name: "Commuter".to_owned(),
                gears: 8,
            }],
        }
    );
}

#[derive(Debug, Deserialize, PartialEq)]
struct BorrowedLabel<'a> {
    label: &'a str,
}

#[test]
fn type_erased_deserializer_preserves_borrowed_strings() {
    let yaml = "label: workshop\n";

    let label: BorrowedLabel<'_> =
        deserialize_erased(yaml).expect("borrowed string must deserialize");

    assert_eq!(label, BorrowedLabel { label: "workshop" });
}

#[test]
fn type_erased_deserializer_propagates_yaml_errors() {
    let yaml = "name: Wheel Works\nopen: true\n";

    let error = deserialize_erased::<Workshop>(yaml)
        .expect_err("missing field must fail through the erased deserializer");

    assert!(
        error.to_string().contains("missing field `bikes`"),
        "{error}"
    );
}

#[test]
fn type_erased_deserializer_expands_explicit_merge_tag() {
    let yaml = "!!merge <<: {a: 1, b: 2}\nown: 3\n";

    let map: BTreeMap<String, i32> =
        deserialize_erased(yaml).expect("explicit merge tag must survive type erasure");

    assert_eq!(
        map,
        BTreeMap::from([
            ("a".to_owned(), 1),
            ("b".to_owned(), 2),
            ("own".to_owned(), 3),
        ])
    );
}
