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
