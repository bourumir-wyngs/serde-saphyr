use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Address {
    lines: String,
    city: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Wrapper {
    address: Address,
}

// Regression test: when emitting a literal block scalar ("|") as a mapping value
// under a nested mapping key, the body lines must be indented under the correct
// mapping base, not using depth+1 unconditionally.
#[test]
fn nested_mapping_block_scalar_body_is_correctly_indented() -> anyhow::Result<()> {
    let w = Wrapper {
        address: Address {
            lines: "line A\nline B\n".to_string(),
            city: "Town".to_string(),
        },
    };

    let yaml = serde_saphyr::to_string(&w)?;

    // Expect the following shape (indentation significant):
    // address:
    //   lines: |
    //     line A
    //     line B
    //   city: Town
    assert!(yaml.contains("address:\n  lines: |\n    line A\n    line B\n  city: Town\n"), "Unexpected YAML:\n{yaml}");

    // And ensure it round-trips
    let w2: Wrapper = serde_saphyr::from_str(&yaml)?;
    assert_eq!(w, w2);

    Ok(())
}
