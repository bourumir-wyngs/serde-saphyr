#![cfg(feature = "garde")]

use garde::Path;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Root {
    users: Vec<User>,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

#[test]
fn garde_path_map_records_locations_for_nested_paths() {
    let yaml = "\
users:\n\
  - name: a\n\
  - name: b\n\
  - name: c\n\
  - name: d\n";

    let (root, map) =
        serde_saphyr::from_str_with_options_and_path_map::<Root>(yaml, Default::default())
            .unwrap();

    assert_eq!(root.users.len(), 4);
    assert_eq!(root.users[3].name, "d");

    let p = Path::new("users").join(3usize).join("name");
    let loc = map.get(&p).expect("missing garde path location");
    assert_eq!(loc.line(), 5);
    // `serde-saphyr` records the *reference* location (use-site), consistent with `Spanned<T>`.
    // For `name: d` this points at the `:` token rather than the first character of the scalar.
    assert_eq!(loc.column(), 9);
}
