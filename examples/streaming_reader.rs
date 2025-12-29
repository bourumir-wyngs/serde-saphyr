use std::io::stdin;

fn main() -> anyhow::Result<()> {
    println!(
        "This program reads YAML files from console. It parses as you type.\
    Type in any valid YAML code. Use --- to separate the documents. \
    Some content of the next document is read before the current parsed document is emitted
    "
    );

    let mut stdin = stdin();

    let iterator: Box<dyn Iterator<Item = Result<serde_json::Value, _>>> =
        serde_saphyr::read(&mut stdin);
    for document in iterator {
        match document {
            Ok(document) => {
                println!("\n** RECEIVED **\n{:#?}\n ******", document);
            }
            Err(error) => {
                println!("\n** ERROR **\n{:#?}\n", error);
                return Err(error.into());
            }
        }
    }

    Ok(())
}
