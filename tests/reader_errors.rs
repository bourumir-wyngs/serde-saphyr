#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::Error;
use std::io::{self, Read};

struct ErrorAfterPrefix {
    prefix: &'static [u8],
    position: usize,
    error: Option<io::Error>,
}

impl ErrorAfterPrefix {
    fn new(prefix: &'static [u8], error: io::Error) -> Self {
        Self {
            prefix,
            position: 0,
            error: Some(error),
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

        if let Some(error) = self.error.take() {
            return Err(error);
        }

        Ok(0)
    }
}

fn injected_error(kind: io::ErrorKind) -> io::Error {
    io::Error::new(kind, "injected reader failure")
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
        injected_error(io::ErrorKind::ConnectionReset),
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
        injected_error(io::ErrorKind::ConnectionReset),
    ))
    .expect_err("late reader failure must be returned");

    assert_injected_io_error(&error, io::ErrorKind::ConnectionReset);
}

#[test]
fn from_reader_does_not_treat_reader_unexpected_eof_as_clean_eof() {
    let error = serde_saphyr::from_reader::<_, Simple>(ErrorAfterPrefix::new(
        b"id: 7\n...\n",
        injected_error(io::ErrorKind::UnexpectedEof),
    ))
    .expect_err("reader-generated UnexpectedEof must be returned");

    assert_injected_io_error(&error, io::ErrorKind::UnexpectedEof);
}

#[test]
fn from_reader_preserves_raw_os_error() {
    const RAW_OS_ERROR: i32 = 12_345;

    let error = serde_saphyr::from_reader::<_, Simple>(ErrorAfterPrefix::new(
        b"id: 7\n...\n",
        io::Error::from_raw_os_error(RAW_OS_ERROR),
    ))
    .expect_err("late reader failure must be returned");

    match error.without_snippet() {
        Error::IOError { cause } => assert_eq!(cause.raw_os_error(), Some(RAW_OS_ERROR)),
        other => panic!("expected reader I/O error, got {other:?}"),
    }
}

#[derive(Debug)]
struct InjectedCause {
    marker: &'static str,
}

impl std::fmt::Display for InjectedCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.marker)
    }
}

impl std::error::Error for InjectedCause {}

#[test]
fn from_reader_preserves_custom_io_error_cause() {
    let error = serde_saphyr::from_reader::<_, Simple>(ErrorAfterPrefix::new(
        b"id: 7\n...\n",
        io::Error::other(InjectedCause {
            marker: "custom injected cause",
        }),
    ))
    .expect_err("late reader failure must be returned");

    match error.without_snippet() {
        Error::IOError { cause } => {
            let inner = cause
                .get_ref()
                .and_then(|error| error.downcast_ref::<InjectedCause>())
                .expect("the original custom error must remain directly inspectable");
            assert_eq!(inner.marker, "custom injected cause");
        }
        other => panic!("expected reader I/O error, got {other:?}"),
    }
}
