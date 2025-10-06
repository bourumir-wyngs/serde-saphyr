use serde::Deserialize;
use indoc::indoc;

#[derive(Debug, PartialEq, Deserialize)]
struct Point {
    x: i32,
}

#[test]
fn test_stream_deserializer() {
    let yaml = indoc!("---\nx: 1\n---\nx: 2\n");
    let mut stream = serde_saphyr::Deserializer::from_str(yaml).into_iter::<Point>();
    assert_eq!(stream.next().unwrap().unwrap(), Point { x: 1 });
    assert_eq!(stream.next().unwrap().unwrap(), Point { x: 2 });
    assert!(stream.next().is_none());
}
