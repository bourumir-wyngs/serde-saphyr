#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::Error;
use std::io::{self, Read};

struct ErrorAfterPrefix {
    prefix: &'static [u8],
    position: usize,
    error_kind: io::ErrorKind,
    emitted_error: bool,
}

impl ErrorAfterPrefix {
    fn new(prefix: &'static [u8], error_kind: io::ErrorKind) -> Self {
        Self {
            prefix,
            position: 0,
            error_kind,
            emitted_error: false,
        }
    }
}

impl Read for ErrorAfterPrefix {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        if self.position < self.prefix.len() {
            let remaining = &self.prefix[self.position..];
            let count = remaining.len().min(output.len());
            output[..count].copy_from_slice(&remaining[..count]);
            self.position += count;
            return Ok(count);
        }

        if !self.emitted_error {
            self.emitted_error = true;
            return Err(io::Error::new(self.error_kind, "injected reader failure"));
        }

        Ok(0)
    }
}

fn assert_injected_io_error(error: &Error, expected_kind: io::ErrorKind) {
    match error.without_snippet() {
        Error::IOError { cause } => {
            assert_eq!(cause.kind(), expected_kind);
            assert!(cause.to_string().contains("injected reader failure"));
        }
        other => panic!("expected reader I/O error, got {other:?}"),
    }
}

#[test]
fn from_reader_propagates_io_error() {
    let error = serde_saphyr::from_reader::<_, serde::de::IgnoredAny>(ErrorAfterPrefix::new(
        b"",
        io::ErrorKind::ConnectionReset,
    ))
    .expect_err("reader failure must be returned");

    assert_injected_io_error(&error, io::ErrorKind::ConnectionReset);
}

#[derive(Debug, Deserialize)]
struct Simple {
    #[allow(dead_code)]
    id: u8,
}

#[test]
fn from_reader_does_not_swallow_io_error_after_document_end() {
    let error = serde_saphyr::from_reader::<_, Simple>(ErrorAfterPrefix::new(
        b"id: 7\n...\n",
        io::ErrorKind::ConnectionReset,
    ))
    .expect_err("late reader failure must be returned");

    assert_injected_io_error(&error, io::ErrorKind::ConnectionReset);
}

#[test]
fn from_reader_does_not_treat_reader_unexpected_eof_as_clean_eof() {
    let error = serde_saphyr::from_reader::<_, Simple>(ErrorAfterPrefix::new(
        b"id: 7\n...\n",
        io::ErrorKind::UnexpectedEof,
    ))
    .expect_err("reader-generated UnexpectedEof must be returned");

    assert_injected_io_error(&error, io::ErrorKind::UnexpectedEof);
}
