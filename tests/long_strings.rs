use anyhow::Context;
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
fn yaml_long_strings() -> anyhow::Result<()> {
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
    )?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long, test.long);
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct FooLs {
    a: i32,
    b: bool,
    short: String,
    long: LitString,
}

#[test]
fn yaml_long_strings_ls() -> anyhow::Result<()> {
    let reference = FooLs {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: LitString("A".repeat(200)),
    };

    let serialized = yaml::to_string(&reference)?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long.0, test.long);
    Ok(())
}

#[test]
fn prefer_block_scalars_must_not_hard_break_long_token() -> anyhow::Result<()> {
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
                .context("Expected a folded scalar body line after 'long: >'")?;
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
fn yaml_long_strings_2() -> anyhow::Result<()> {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "A".repeat(200),
    };
    let serialized = yaml::to_string(&reference)?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long, test.long);
    Ok(())
}

#[test]
fn yaml_long_strings_with_breaks() -> anyhow::Result<()> {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "AB CD".repeat(200),
    };
    let serialized = yaml::to_string(&reference)?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long, test.long);
    Ok(())
}

#[test]
fn yaml_long_strings_with_double_breaks() -> anyhow::Result<()> {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "AB  CD".repeat(200),
    };
    let serialized = yaml::to_string(&reference)?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long, test.long);
    Ok(())
}

#[test]
fn yaml_long_strings_with_triple_breaks() -> anyhow::Result<()> {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "AB   CD".repeat(200),
    };
    let serialized = yaml::to_string(&reference)?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long, test.long);
    Ok(())
}

#[test]
fn yaml_long_strings_with_var_breaks() -> anyhow::Result<()> {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "Aaaaaaa Bbbbbbbb  Ccccccc   Dddddd Eeeeee".repeat(200),
    };
    let serialized = yaml::to_string(&reference)?;
    let test: Foo = yaml::from_str(serialized.as_str())?;
    assert_eq!(reference.long, test.long);
    Ok(())
}

#[test]
fn folded_wrap_can_preserve_multi_space_runs_by_emitting_trailing_spaces() -> anyhow::Result<()> {
    // When wrapping a folded block scalar (`>`), a single line break is folded into a single
    // space on parse. To preserve runs of multiple spaces without starting the next YAML line
    // with spaces (which can trigger YAML "more-indented" semantics), the serializer may emit
    // (run_len - 1) spaces at end-of-line and consume the entire whitespace run.
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "AAAAA  BBBBB".repeat(50),
    };

    let mut serialized = String::new();
    yaml::to_fmt_writer_with_options(
        &mut serialized,
        &reference,
        yaml::SerializerOptions {
            prefer_block_scalars: true,
            folded_wrap_chars: 10,
            ..Default::default()
        },
    )?;

    assert!(
        serialized.contains("long: >"),
        "Unexpected YAML (expected folded block scalar):\n{serialized}"
    );

    // Extract body lines of the folded scalar and verify:
    // - none of them start with extra whitespace beyond indentation,
    // - at least one ends with a trailing space (the "extra space before line break").
    let mut lines = serialized.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("long: >") {
            break;
        }
    }

    let mut saw_trailing_space = false;
    while let Some(&next) = lines.peek() {
        // Body lines are indented by two spaces for this struct.
        if !next.starts_with("  ") {
            break;
        }
        let body_line = lines.next().context("Expected a folded scalar body line")?;
        let content = body_line
            .strip_prefix("  ")
            .context("Expected folded scalar body line to start with indentation")?;
        if content.starts_with(char::is_whitespace) {
            anyhow::bail!(
                "Folded scalar body line started with whitespace beyond indentation: {body_line:?}\nYAML:\n{serialized}"
            );
        }
        if content.ends_with(' ') {
            saw_trailing_space = true;
        }
    }
    assert!(
        saw_trailing_space,
        "Expected at least one wrapped folded-scalar line to end with a space. YAML:\n{serialized}"
    );

    let decoded: Foo = yaml::from_str(&serialized)?;
    assert_eq!(decoded.long, reference.long);
    Ok(())
}
