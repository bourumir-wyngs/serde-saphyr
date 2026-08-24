#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[typetag::serde(tag = "kind")]
trait Bike {
    fn name(&self) -> &str;

    fn engine_power(&self) -> Option<f32> {
        None
    }

    fn is_ebike(&self) -> Option<bool> {
        None
    }
}

#[derive(Serialize, Deserialize)]
struct Motorbike {
    engine_power: f32,
}

#[typetag::serde]
impl Bike for Motorbike {
    fn name(&self) -> &str {
        "motorbike"
    }

    fn engine_power(&self) -> Option<f32> {
        Some(self.engine_power)
    }
}

#[derive(Serialize, Deserialize)]
struct Bicycle {
    ebike: bool,
}

#[typetag::serde]
impl Bike for Bicycle {
    fn name(&self) -> &str {
        "bicycle"
    }

    fn is_ebike(&self) -> Option<bool> {
        Some(self.ebike)
    }
}

fn roundtrip_bike(bike: Box<dyn Bike>, expected_yaml: &str) -> Box<dyn Bike> {
    let yaml = serde_saphyr::to_string(&bike).expect("typetag serialization must succeed");
    assert_eq!(yaml, expected_yaml);

    serde_saphyr::from_str(&yaml).expect("typetag deserialization must succeed")
}

#[test]
fn internally_tagged_trait_objects_roundtrip() {
    let motorbike: Box<dyn Bike> = Box::new(Motorbike {
        engine_power: 600.0,
    });
    let restored = roundtrip_bike(motorbike, "kind: Motorbike\nengine_power: 600.0\n");
    assert_eq!(restored.name(), "motorbike");
    assert_eq!(restored.engine_power(), Some(600.0));
    assert_eq!(restored.is_ebike(), None);

    let bicycle: Box<dyn Bike> = Box::new(Bicycle { ebike: true });
    let restored = roundtrip_bike(bicycle, "kind: Bicycle\nebike: true\n");
    assert_eq!(restored.name(), "bicycle");
    assert_eq!(restored.engine_power(), None);
    assert_eq!(restored.is_ebike(), Some(true));
}

#[derive(Serialize, Deserialize)]
struct Garage {
    bikes: Vec<Box<dyn Bike>>,
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

impl<'de> Deserialize<'de> for Owners {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OwnersVisitor;

        impl<'de> Visitor<'de> for OwnersVisitor {
            type Value = Owners;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a mapping from bikes to owner names")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut owners = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some(entry) = map.next_entry()? {
                    owners.push(entry);
                }
                Ok(Owners(owners))
            }
        }

        deserializer.deserialize_map(OwnersVisitor)
    }
}

#[derive(Serialize, Deserialize)]
struct BikeOwners {
    owners: Owners,
}

#[derive(Serialize)]
struct BorrowedBike<'a> {
    bike: &'a dyn Bike,
}

#[test]
fn borrowed_trait_object_serializes_when_nested() {
    let bike = Motorbike { engine_power: 50.0 };
    let borrowed = BorrowedBike { bike: &bike };

    let yaml = serde_saphyr::to_string(&borrowed).expect("borrowed bike must serialize");
    assert_eq!(yaml, "bike:\n  kind: Motorbike\n  engine_power: 50.0\n");
}

#[test]
fn nested_trait_object_collection_roundtrips() {
    let garage = Garage {
        bikes: vec![
            Box::new(Motorbike {
                engine_power: 125.0,
            }),
            Box::new(Bicycle { ebike: false }),
        ],
    };

    let yaml = serde_saphyr::to_string(&garage).expect("garage serialization must succeed");
    let restored: Garage =
        serde_saphyr::from_str(&yaml).expect("garage deserialization must succeed");

    assert_eq!(restored.bikes.len(), 2);
    assert_eq!(restored.bikes[0].name(), "motorbike");
    assert_eq!(restored.bikes[0].engine_power(), Some(125.0));
    assert_eq!(restored.bikes[1].name(), "bicycle");
    assert_eq!(restored.bikes[1].is_ebike(), Some(false));
}

#[test]
fn owners_roundtrip_with_bike_as_the_mapping_key() {
    let document = BikeOwners {
        owners: Owners(vec![(
            Box::new(Bicycle { ebike: true }),
            vec!["Alice".to_owned(), "Bob".to_owned()],
        )]),
    };

    let yaml = serde_saphyr::to_string(&document).expect("owners serialization must succeed");
    assert_eq!(
        yaml,
        "owners:\n  ? kind: Bicycle\n    ebike: true\n  :\n  - Alice\n  - Bob\n"
    );
    let restored: BikeOwners =
        serde_saphyr::from_str(&yaml).expect("owners deserialization must succeed");

    assert_eq!(restored.owners.0.len(), 1);
    let (bike, owners) = &restored.owners.0[0];
    assert_eq!(bike.name(), "bicycle");
    assert_eq!(bike.is_ebike(), Some(true));
    assert_eq!(owners, &["Alice", "Bob"]);
}

fn bike_deserialization_error(yaml: &str) -> String {
    match serde_saphyr::from_str::<Box<dyn Bike>>(yaml) {
        Ok(_) => panic!("invalid typetag input unexpectedly deserialized"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn internally_tagged_trait_object_rejects_unknown_and_missing_tags() {
    let unknown = bike_deserialization_error("kind: Scooter\nwheels: 2\n");
    assert!(unknown.contains("unknown variant `Scooter`"), "{unknown}");

    let missing = bike_deserialization_error("engine_power: 600.0\n");
    assert!(missing.contains("missing field `kind`"), "{missing}");
}

mod externally_tagged {
    use serde::{Deserialize, Serialize};

    #[typetag::serde]
    trait Brake {
        fn pistons(&self) -> u8;
    }

    #[derive(Serialize, Deserialize)]
    struct DiscBrake {
        pistons: u8,
    }

    #[typetag::serde(name = "disc")]
    impl Brake for DiscBrake {
        fn pistons(&self) -> u8 {
            self.pistons
        }
    }

    #[test]
    fn custom_named_trait_object_roundtrips() {
        let brake: Box<dyn Brake> = Box::new(DiscBrake { pistons: 4 });
        let yaml = serde_saphyr::to_string(&brake).expect("serialization must succeed");
        assert_eq!(yaml, "disc:\n  pistons: 4\n");

        let restored: Box<dyn Brake> =
            serde_saphyr::from_str(&yaml).expect("deserialization must succeed");
        assert_eq!(restored.pistons(), 4);
    }

    #[test]
    fn multiple_variant_entries_are_rejected() {
        let yaml = "disc:\n  pistons: 4\nanother:\n  pistons: 2\n";
        let error = match serde_saphyr::from_str::<Box<dyn Brake>>(yaml) {
            Ok(_) => panic!("multiple externally tagged entries unexpectedly deserialized"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("expected mapping end"), "{error}");
    }
}

mod adjacently_tagged {
    use serde::{Deserialize, Serialize};

    #[typetag::serde(
        tag = "kind",
        content = "configuration",
        default_variant = "headlight",
        deny_unknown_fields
    )]
    trait Light {
        fn lumens(&self) -> u16;
    }

    #[derive(Serialize, Deserialize)]
    struct Headlight {
        lumens: u16,
    }

    #[typetag::serde(name = "headlight")]
    impl Light for Headlight {
        fn lumens(&self) -> u16 {
            self.lumens
        }
    }

    #[test]
    fn custom_named_trait_object_roundtrips() {
        let light: Box<dyn Light> = Box::new(Headlight { lumens: 800 });
        let yaml = serde_saphyr::to_string(&light).expect("serialization must succeed");
        assert_eq!(yaml, "kind: headlight\nconfiguration:\n  lumens: 800\n");

        let restored: Box<dyn Light> =
            serde_saphyr::from_str(&yaml).expect("deserialization must succeed");
        assert_eq!(restored.lumens(), 800);
    }

    #[test]
    fn default_variant_is_used_when_tag_is_missing() {
        let yaml = "configuration:\n  lumens: 600\n";
        let restored: Box<dyn Light> =
            serde_saphyr::from_str(yaml).expect("default variant must deserialize");
        assert_eq!(restored.lumens(), 600);
    }

    #[test]
    fn unknown_fields_are_rejected_when_configured() {
        let yaml = "kind: headlight\nconfiguration:\n  lumens: 800\nunexpected: true\n";
        let error = match serde_saphyr::from_str::<Box<dyn Light>>(yaml) {
            Ok(_) => panic!("unknown field unexpectedly accepted"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("unknown field `unexpected`"), "{error}");
    }
}

mod non_struct_implementation {
    #[typetag::serde(tag = "kind", content = "value")]
    trait Component {
        fn value(&self) -> u8;
    }

    #[typetag::serde(name = "gear_count")]
    impl Component for u8 {
        fn value(&self) -> u8 {
            *self
        }
    }

    #[test]
    fn primitive_trait_implementation_roundtrips() {
        let gear_count: Box<dyn Component> = Box::new(12_u8);
        let yaml = serde_saphyr::to_string(&gear_count).expect("serialization must succeed");
        assert_eq!(yaml, "kind: gear_count\nvalue: 12\n");

        let restored: Box<dyn Component> =
            serde_saphyr::from_str(&yaml).expect("deserialization must succeed");
        assert_eq!(restored.value(), 12);
    }
}
