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
        value_array: []
    };

    let mut s: String = String::new();
    serde_saphyr::to_fmt_writer(&mut s, &ea)?;
    assert_eq!("value_vec: []\nvalue_array: []\n", s);

    Ok(())
}