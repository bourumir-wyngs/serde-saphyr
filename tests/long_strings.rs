use serde::{Deserialize, Serialize};
use serde_saphyr as yaml;
use serde_saphyr::LitString;

#[derive(Serialize, Deserialize)]
struct Foo {
    a: i32,
    b: bool,
    short: String,
    long: String,
}

#[test]
fn yaml_long_strings() {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "A".repeat(200),
    };

    let mut serialized = String::new();
    yaml::to_fmt_writer_with_options(
        &mut serialized,
        &reference,
        yaml::SerializerOptions {
            prefer_block_scalars: false,
            ..Default::default()
        },
    )
    .expect("Unable to serialize my struct!");
    let test: Foo = yaml::from_str(serialized.as_str()).expect("Unable to deserialize my struct!");
    assert_eq!(reference.long, test.long);
}

#[derive(Serialize, Deserialize)]
struct FooLs {
    a: i32,
    b: bool,
    short: String,
    long: LitString,
}

#[test]
fn yaml_long_strings_ls() {
    let reference = FooLs {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: LitString("A".repeat(200)),
    };

    let serialized = yaml::to_string(&reference).expect("Unable to serialize my struct!");
    let test: Foo = yaml::from_str(serialized.as_str()).expect("Unable to deserialize my struct!");
    assert_eq!(reference.long.0, test.long);
}

#[test]
fn prefer_block_scalars_must_not_hard_break_long_token() -> anyhow::Result<()> {
    // Regression: with prefer_block_scalars enabled, long single-token strings (no spaces)
    // must not be hard-broken in folded block style, because YAML folding would insert
    // spaces on parse and change the value.
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "A".repeat(200),
    };

    let mut serialized = String::new();
    yaml::to_fmt_writer_with_options(
        &mut serialized,
        &reference,
        yaml::SerializerOptions {
            prefer_block_scalars: true,
            ..Default::default()
        },
    )?;

    // Ensure the long field was emitted as a folded block scalar (default auto behavior).
    assert!(
        serialized.contains("long: >"),
        "Unexpected YAML (expected folded block scalar):\n{serialized}"
    );

    // Body of the folded scalar must be a single (potentially long) line: no inserted newlines.
    // We look for the header line, then ensure the following indented content line is not split.
    let mut lines = serialized.lines();
    let mut found = false;
    while let Some(line) = lines.next() {
        if line.starts_with("long: >") {
            let body = lines
                .next()
                .expect("Expected a folded scalar body line after 'long: >'");
            assert_eq!(body, format!("  {}", "A".repeat(200)));
            found = true;
            break;
        }
    }
    assert!(
        found,
        "Did not find a 'long: >' folded scalar header in YAML:\n{serialized}"
    );

    let decoded: Foo = yaml::from_str(&serialized)?;
    assert_eq!(decoded.long, reference.long);
    Ok(())
}

#[test]
fn yaml_long_strings_2() {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "A".repeat(200)
    };
    let serialized = yaml::to_string(&reference).expect("Unable to serialize my struct!");
    let test: Foo = yaml::from_str(serialized.as_str()).expect("Unable to deserialize my struct!");
    assert_eq!(reference.long,test.long);
}
