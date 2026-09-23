use indoc::indoc;
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct Point {
    x: i32,
}

#[test]
fn test_stream_deserializer() -> anyhow::Result<()> {
    let yaml = indoc!("---\nx: 1\n---\nx: 2\n");
    let mut stream = serde_saphyr::from_str_multiple::<Point, Vec<_>>(yaml)?.into_iter();
    assert_eq!(stream.next().unwrap(), Point { x: 1 });
    assert_eq!(stream.next().unwrap(), Point { x: 2 });
    assert!(stream.next().is_none());
    Ok(())
}
