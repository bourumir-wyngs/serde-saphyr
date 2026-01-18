use std::collections::HashMap;

#[test]
fn yaml_2xxw_unordered_set_as_map_with_nulls() {
    let yaml = "--- !!set\n? Mark McGwire\n? Sammy Sosa\n? Ken Griff\n";

    // Represent null values as Option<String> = None
    let m: HashMap<String, Option<String>> =
        serde_saphyr::from_str(yaml).expect("failed to parse !!set as map");

    assert!(m.contains_key("Mark McGwire"));
    assert!(m.contains_key("Sammy Sosa"));
    assert!(m.contains_key("Ken Griff"));
    assert_eq!(m.get("Mark McGwire").unwrap(), &None);
    assert_eq!(m.get("Sammy Sosa").unwrap(), &None);
    assert_eq!(m.get("Ken Griff").unwrap(), &None);
    assert_eq!(m.len(), 3);
}
