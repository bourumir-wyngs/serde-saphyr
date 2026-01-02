use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")] // Rust in snake_case, YAML in camelCase.
struct AB {
    // Just defined here (we validate `second_string` only).
    #[allow(dead_code)]
    first_string: String,

    #[validate(length(min = 2))]
    second_string: String,
}

fn main() {
    let yaml = r#"
        firstString: &A "x"
        secondString: *A
   "#;

    let err = serde_saphyr::from_str_with_options_validate::<AB>(yaml, Default::default())
        .expect_err("must fail validation");

    // Field in error message in camelCase (as in YAML).
    eprintln!("{err}");
}
