use std::io::stdin;

fn main() -> anyhow::Result<()> {
    println!(
        "This program reads YAML files from console. It parses as you type.\
    Type in any valid YAML code. Use --- to separate the documents. \
    Some content of the next document is read before the current parsed document is emitted
    "
    );

    let mut stdin = stdin();

    let iterator = serde_saphyr::read::<_, serde_json::Value>(&mut stdin);
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
