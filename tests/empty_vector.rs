#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Serialize;

#[test]
fn serialize_empty_vec() -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Ea {
        value_vec: Vec<usize>,
        value_array: [f32; 0],
    }

    let ea = Ea {
        value_vec: Vec::new(),
        value_array: [],
    };

    let mut s: String = String::new();
    serde_saphyr::to_fmt_writer(&mut s, &ea)?;
    assert_eq!("value_vec: []\nvalue_array: []\n", s);

    Ok(())
}

#[test]
fn empty_vec_as_map_value_does_not_leak_state_when_braces_disabled() -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Wrapper {
        a: Vec<u8>,
    }

    let value = vec![Wrapper { a: Vec::new() }, Wrapper { a: vec![1] }];
    let options = serde_saphyr::ser_options! { empty_as_braces: false };

    let mut out = String::new();
    serde_saphyr::to_fmt_writer_with_options(&mut out, &value, options)?;

    assert_eq!("- a:\n- a:\n  - 1\n", out);
    Ok(())
}

#[test]
fn empty_vec_after_nested_compact_sequence_round_trips() -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Root {
        tpm_attestation: Tpm,
    }

    #[derive(Serialize)]
    struct Tpm {
        quote: Quote,
        event_log: Vec<u8>,
        instance_info: Option<Vec<u8>>,
    }

    #[derive(Serialize)]
    struct Quote {
        pcrs: Vec<Vec<u8>>,
    }

    let value = Root {
        tpm_attestation: Tpm {
            quote: Quote {
                pcrs: vec![vec![0, 0]],
            },
            event_log: vec![],
            instance_info: None,
        },
    };

    let yaml = serde_saphyr::to_string(&value)?;

    assert!(
        yaml.contains("  event_log: []\n"),
        "empty sequence must stay on the map-value line or be indented as a nested value:\n{yaml}"
    );
    assert!(
        !yaml.contains("  event_log:\n  []"),
        "empty sequence rendered at key indentation is invalid YAML:\n{yaml}"
    );

    let _: serde_json::Value = serde_saphyr::from_str(&yaml)?;
    Ok(())
}
