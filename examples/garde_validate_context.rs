use garde::Validate;
use serde::Deserialize;

#[derive(Default)]
struct ValidationContext {
    allowed_string: String,
}

fn validate_extra(value: &String, context: &ValidationContext) -> garde::Result {
    if value == &context.allowed_string {
        return Ok(());
    }
    Err(garde::Error::new(format!(
        "value '{}' does not match allowed string '{}'",
        value, context.allowed_string
    )))
}

#[derive(Debug, Deserialize, Validate)]
#[garde(context(ValidationContext))]
struct AB {
    // Validate with context
    #[garde(custom(validate_extra))]
    a_string: String,
}

fn main() {
    let yaml = r#"
        a_string: "y"
   "#;

    let context = ValidationContext {
        allowed_string: "test".to_string(),
    };

    let err =
        serde_saphyr::from_str_with_options_context_valid::<AB>(yaml, Default::default(), &context)
            .expect_err("must fail validation");

    // Field in error message in camelCase (as in YAML).
    eprintln!("{err}");
}
