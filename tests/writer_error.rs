use std::io;
use std::io::Write;
use serde::Serialize;

#[derive(Serialize)]
struct Payload {
    a: u8,
}

/// Writer that always returns an IO error on any write.
struct AlwaysErrorWriter;

impl Write for AlwaysErrorWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "boom from writer"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "flush boom"))
    }
}

#[test]
fn to_io_writer_propagates_io_error() {
    let mut w = AlwaysErrorWriter;
    let val = Payload { a: 1 };

    let err = serde_saphyr::to_io_writer(&mut w, &val).expect_err("expected IO error");

    // Ensure the error is the IO variant and we can inspect the inner io::Error
    match &err {
        serde_saphyr::ser_error::Error::IO { error } => {
            assert_eq!(error.kind(), io::ErrorKind::Other);
            // Check message content as much as possible
            let msg = error.to_string();
            assert!(msg.contains("boom"), "unexpected IO error message: {msg}");
        }
        other => panic!(
            "expected IO error variant, got: {} ({other:?})",
            other
        ),
    }

    // Also check Display of top-level error includes the IO message prefix
    let display = format!("{}", err);
    assert!(display.starts_with("I/O error:"), "unexpected display: {display}");
}
