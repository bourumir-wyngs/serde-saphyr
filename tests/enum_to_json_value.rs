use serde::Deserialize;

#[derive(Deserialize, Debug)]
enum AnonymousValues {
    Unit,
    Newtype(i32),
    Tuple(i32, String),
}

#[derive(Deserialize, Debug)]
enum NamedValues {
    Struct { x: i32, y: String },
    Empty {},
}

#[test]
fn print_enum_deserialization_yaml_to_json_value() {
    let yaml_inputs: Vec<(&str, &str)> = vec![
        ("AnonymousValues::Unit", "Unit"),
        ("AnonymousValues::Newtype", "Newtype: 42"),
        ("AnonymousValues::Tuple", "Tuple:\n  - 1\n  - hello"),
        ("NamedValues::Struct", "Struct:\n  x: 10\n  y: world"),
        ("NamedValues::Empty", "Empty: {}"),
        // Tagged with !
        ("!Unit", "!Unit"),
        ("!Newtype", "!Newtype 42"),
        ("!Tuple sequence", "!Tuple\n  - 1\n  - hello"),
        ("!Struct mapping", "!Struct\n  x: 10\n  y: world"),
        ("!Empty mapping", "!Empty {}"),
    ];

    for (label, yaml) in &yaml_inputs {
        let result: Result<serde_json::Value, _> = serde_saphyr::from_str(yaml);
        match result {
            Ok(json) => println!("{label}: yaml={yaml:?} => {json}"),
            Err(e) => println!("{label}: yaml={yaml:?} => ERROR: {e}"),
        }
    }
}
