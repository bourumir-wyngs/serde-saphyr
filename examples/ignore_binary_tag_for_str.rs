use serde::Deserialize;

#[derive(Deserialize)]
struct ContainsString {
    name: String,
}

// see https://github.com/j178/prek/issues/1102
fn main() -> Result<(), serde_saphyr::Error> {
    let content = "name: !!binary H4sIAA==";
    let value: ContainsString = serde_saphyr::from_str_with_options(
        content,
        serde_saphyr::options! {
            ignore_binary_tag_for_string: true
        },
    )?;
    println!("name: {}", value.name); // name: H4sIAA==
    Ok(())
}
