#![cfg(any(feature = "garde", feature = "validator"))]

use serde::Deserialize;
use serde_saphyr::path_map::{PathKey, PathKind};
use serde_saphyr::{Error, Location};
#[cfg(feature = "validator")]
use validator::Validate as _;

#[test]
fn segments_preserve_order_and_distinguish_keys_from_indices() {
    assert!(PathKey::new().segments().next().is_none());
    let path = PathKey::new().join_key("items").join_index(0).join_key("0");
    let segments: Vec<(PathKind, &str)> = path.segments().collect();
    assert_eq!(
        segments,
        [
            (PathKind::Key, "items"),
            (PathKind::Index, "0"),
            (PathKind::Key, "0")
        ]
    );
    assert_ne!(PathKey::new().join_key("0"), PathKey::new().join_index(0));
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
#[serde(rename_all = "camelCase")]
struct Config {
    #[serde(rename = "template")]
    #[cfg_attr(feature = "garde", garde(skip))]
    _template: String,
    #[cfg_attr(feature = "garde", garde(dive))]
    #[cfg_attr(feature = "validator", validate(nested))]
    my_items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
#[serde(rename_all = "camelCase")]
struct Item {
    #[cfg_attr(feature = "garde", garde(length(min = 3)))]
    #[cfg_attr(feature = "validator", validate(length(min = 3)))]
    display_name: String,
}

const YAML: &str = "template: &short x\nmyItems:\n  - displayName: *short\n";

fn coordinates(location: Location) -> (u64, u64) {
    (location.line(), location.column())
}

#[track_caller]
fn assert_accessors(error: &Error) {
    let Error::ValidationError {
        issues, locations, ..
    } = error.without_snippet()
    else {
        panic!("expected validation error, got {error:?}");
    };
    assert_eq!(issues.len(), 1);
    let path = &issues[0].path;
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [
            (PathKind::Key, "my_items"),
            (PathKind::Index, "0"),
            (PathKind::Key, "display_name")
        ]
    );

    let alias = locations
        .locations_for(path)
        .expect("validation field location");
    assert_eq!(coordinates(alias.reference_location), (3, 18));
    assert_eq!(coordinates(alias.defined_location), (1, 18));
    let yaml_path = PathKey::new()
        .join_key("myItems")
        .join_index(0)
        .join_key("displayName");
    assert_eq!(locations.locations_for(&yaml_path), Some(alias));

    // A transformed validation path can extend past the recorded YAML shape.
    assert_eq!(
        locations.locations_for(&path.clone().join_key("missing")),
        Some(alias)
    );
    let parent = PathKey::new().join_key("my_items").join_index(0);
    let parent_location = locations.locations_for(&parent).expect("mapping location");
    assert_eq!(coordinates(parent_location.reference_location), (3, 5));
    assert_eq!(
        locations.locations_for(&parent.join_key("missing").join_key("descendant")),
        Some(parent_location)
    );

    let root = locations
        .locations_for(&PathKey::new())
        .expect("recorded root");
    assert_eq!(coordinates(root.reference_location), (1, 1));
    assert_eq!(root.defined_location, root.reference_location);
    assert_eq!(
        locations.locations_for(&PathKey::new().join_key("absent").join_index(42)),
        Some(root)
    );
}

#[cfg(feature = "garde")]
#[test]
fn garde_paths_and_locations_are_available_for_strings_and_readers() {
    for error in [
        serde_saphyr::from_str_valid::<Config>(YAML).unwrap_err(),
        serde_saphyr::from_reader_valid::<_, Config>(YAML.as_bytes()).unwrap_err(),
    ] {
        assert_accessors(&error);
    }
}

#[cfg(feature = "validator")]
#[test]
fn validator_paths_and_locations_are_available_for_strings_and_readers() {
    for error in [
        serde_saphyr::from_str_validate::<Config>(YAML).unwrap_err(),
        serde_saphyr::from_reader_validate::<_, Config>(YAML.as_bytes()).unwrap_err(),
    ] {
        assert_accessors(&error);
    }
}

#[cfg(feature = "garde")]
#[test]
fn unrecorded_root_has_no_invented_location() {
    #[derive(Debug, Deserialize, garde::Validate)]
    #[garde(transparent)]
    struct Scalar(#[garde(length(min = 3))] String);

    let error = serde_saphyr::from_str_valid::<Scalar>("x").unwrap_err();
    let Error::ValidationError {
        issues, locations, ..
    } = error.without_snippet()
    else {
        panic!("expected validation error, got {error:?}");
    };
    assert!(issues[0].path.segments().next().is_none());
    assert_eq!(locations.locations_for(&issues[0].path), None);
    assert_eq!(
        locations.locations_for(&PathKey::new().join_key("missing")),
        None
    );
}
