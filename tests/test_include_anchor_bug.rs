#![cfg(all(feature = "serialize", feature = "deserialize"))]
#[cfg(feature = "include")]
use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{
    IncludeRequest, IncludeResolveError, InputSource, ResolvedInclude, from_str_with_options,
};

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    cfg: UserConfig,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct UserConfig {
    user: User,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct User {
    name: String,
}

#[cfg(feature = "include")]
#[test]
fn test_alias_resolution_for_anchor_defined_outside_selected_fragment() {
    let yaml = "cfg: !include value.yaml#selected\n";

    let options = serde_saphyr::options! {}.with_include_resolver(|req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "value.yaml#selected" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                source: InputSource::AnchoredText {
                    text: "base: &base\n  name: Alice\n\nother: &other\n  name: Bob\n\ndummy: *other\n\nselected: &selected\n  user: *base\n".to_string(),
                    anchor: "selected".to_string(),
                },
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = from_str_with_options(yaml, options).unwrap();

    assert_eq!(config.cfg.user.name, "Alice");
}
