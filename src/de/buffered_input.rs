//! Streaming, chunked character input helpers.
//!
//! This module provides a small adapter to turn any `std::io::Read` into a
//! streaming iterator of UTF-8 `char`s without loading the entire input into
//! memory. It is used to feed `saphyr_parser` incrementally, while allowing the
//! upstream `encoding_rs_io` to auto-detect and decode common Unicode encodings
//! (BOM-aware), then buffering via `BufReader`.

use encoding_rs_io::DecodeReaderBytesBuilder;
use saphyr_parser::{BorrowedInput, BufferedInput, Input};
use std::cell::{Cell, RefCell};
use std::io::{self, BufReader, Error, Read};
use std::rc::Rc;

type DynReader<'a> = Box<dyn Read + 'a>;
type DynBufReader<'a> = BufReader<DynReader<'a>>;

/// Streaming YAML input backed by a reader.
///
/// This does **not** support zero-copy borrowing of scalar slices, so
/// [`BorrowedInput::slice_borrowed`] always returns `None`.
pub struct ReaderInput<'a>(BufferedInput<ChunkedChars<DynBufReader<'a>>>);

impl<'a> ReaderInput<'a> {
    #[inline]
    pub fn new(inner: BufferedInput<ChunkedChars<DynBufReader<'a>>>) -> Self {
        Self(inner)
    }
}

impl<'a> Input for ReaderInput<'a> {
    #[inline]
    fn lookahead(&mut self, count: usize) {
        self.0.lookahead(count);
    }

    #[inline]
    fn buflen(&self) -> usize {
        self.0.buflen()
    }

    #[inline]
    fn bufmaxlen(&self) -> usize {
        self.0.bufmaxlen()
    }

    #[inline]
    fn raw_read_ch(&mut self) -> char {
        self.0.raw_read_ch()
    }

    #[inline]
    fn raw_read_non_breakz_ch(&mut self) -> Option<char> {
        self.0.raw_read_non_breakz_ch()
    }

    #[inline]
    fn skip(&mut self) {
        self.0.skip();
    }

    #[inline]
    fn skip_n(&mut self, count: usize) {
        self.0.skip_n(count);
    }

    #[inline]
    fn peek(&self) -> char {
        self.0.peek()
    }

    #[inline]
    fn peek_nth(&self, n: usize) -> char {
        self.0.peek_nth(n)
    }
}

impl<'a> BorrowedInput<'a> for ReaderInput<'a> {
    #[inline]
    fn slice_borrowed(&self, _start: usize, _end: usize) -> Option<&'a str> {
        None
    }
}

pub(crate) type ReaderInputError = Rc<RefCell<Option<Error>>>;
pub(crate) type ReaderInputBytesRead = Rc<Cell<usize>>;

pub struct ChunkedChars<R: Read> {
    /// Optional hard cap on total decoded UTF-8 bytes yielded by this iterator.
    max_bytes: Option<usize>,
    /// Running count of decoded bytes yielded so far across all readers sharing this budget.
    bytes_read: ReaderInputBytesRead,
    /// The underlying reader that already yields UTF-8 bytes (typically a
    /// `BufReader<DecodeReaderBytes<...>>`). It is read incrementally.
    reader: R,
    /// Remember IO error, if any, here to report it later. This must be shared,
    /// as otherwise we cannot later reach with Saphyr parser API
    pub(crate) err: Rc<RefCell<Option<Error>>>,
}

impl<R: Read> ChunkedChars<R> {
    pub fn new(
        reader: R,
        max_bytes: Option<usize>,
        err: Rc<RefCell<Option<Error>>>,
        bytes_read: ReaderInputBytesRead,
    ) -> Self {
        Self {
            max_bytes,
            bytes_read,
            reader,
            err,
        }
    }
}

impl<R: Read> Iterator for ChunkedChars<R> {
    type Item = char;

    /// Returns the next Unicode scalar value from the stream, or `None` on EOF.
    /// If error occurs, sets the error field that is a shared reference to the
    /// error value, so that the parser can later pick this up.
    fn next(&mut self) -> Option<char> {
        // Read exactly one UTF-8 codepoint (1..=4 bytes) from the underlying reader.
        // No internal buffering: rely on the outer BufReader and decoder.
        let mut buf = [0u8; 4];
        // Read first byte
        if let Err(e) = self.reader.read_exact(&mut buf[..1]) {
            match e.kind() {
                io::ErrorKind::UnexpectedEof => return None, // true EOF
                _ => {
                    self.err.replace(Some(e));
                    return None;
                }
            }
        }
        let first = buf[0];
        let needed = if first < 0x80 {
            1
        } else if first & 0b1110_0000 == 0b1100_0000 {
            2
        } else if first & 0b1111_0000 == 0b1110_0000 {
            3
        } else if first & 0b1111_1000 == 0b1111_0000 {
            4
        } else {
            // Invalid leading byte
            self.err.replace(Some(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid UTF-8 leading byte",
            )));
            return None;
        };

        if needed > 1 {
            let mut read = 0;
            while read < needed - 1 {
                match self.reader.read(&mut buf[1 + read..needed]) {
                    Ok(0) => {
                        self.err.replace(Some(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "unexpected EOF in middle of UTF-8 codepoint",
                        )));
                        return None;
                    }
                    Ok(n) => read += n,
                    Err(e) => {
                        self.err.replace(Some(e));
                        return None;
                    }
                }
            }
        }

        // Enforce byte limit if configured
        let add = needed;
        let total_bytes = self.bytes_read.get();
        if let Some(limit) = self.max_bytes {
            let new_total = total_bytes.saturating_add(add);
            if new_total > limit {
                self.err.replace(Some(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    format!("input size limit of {limit} bytes exceeded"),
                )));
                return None;
            }
            self.bytes_read.set(new_total);
        } else {
            self.bytes_read.set(total_bytes.saturating_add(add));
        }

        // Validate assembled bytes as UTF-8 and extract the char
        match std::str::from_utf8(&buf[..needed]) {
            Ok(s) => s.chars().next(),
            Err(e) => {
                self.err
                    .replace(Some(io::Error::new(io::ErrorKind::InvalidData, e)));
                None
            }
        }
    }
}

/// Creates buffered input and returns both input and reference to the variable
/// holding the possible error. We cannot otherwise later reach our ChunkedChars.
pub fn buffered_input_from_reader_with_limit<'a, R: Read + 'a>(
    reader: R,
    max_bytes: Option<usize>,
) -> (ReaderInput<'a>, ReaderInputError, ReaderInputBytesRead) {
    let error: ReaderInputError = Rc::new(RefCell::new(None));
    let bytes_read: ReaderInputBytesRead = Rc::new(Cell::new(0));
    let input = buffered_input_from_reader_with_limit_shared(
        reader,
        max_bytes,
        error.clone(),
        bytes_read.clone(),
    );
    (input, error, bytes_read)
}

/// Like [`buffered_input_from_reader_with_limit`] but uses an existing shared error cell.
///
/// This is used to ensure IO errors from nested include readers are observable by the
/// top-level consumer.
pub fn buffered_input_from_reader_with_limit_shared<'a, R: Read + 'a>(
    reader: R,
    max_bytes: Option<usize>,
    error: ReaderInputError,
    bytes_read: ReaderInputBytesRead,
) -> ReaderInput<'a> {
    // Auto-detect encoding (BOM or guess), decode to UTF-8 on the fly.
    let decoder = DecodeReaderBytesBuilder::new()
        .encoding(None) // None = sniff BOM / use heuristics; set Some(encoding) to force
        .bom_override(true)
        .build(reader);

    let br = BufReader::new(Box::new(decoder) as DynReader<'a>);
    let char_iter = ChunkedChars::new(br, max_bytes, error, bytes_read);
    ReaderInput::new(BufferedInput::new(char_iter))
}

#[cfg(test)]
mod tests {
    use crate::buffered_input::ReaderInput;
    use crate::buffered_input::buffered_input_from_reader_with_limit;
    use saphyr_parser::{Event, Parser};
    use std::io::{Cursor, Read};

    pub fn buffered_input_from_reader<'a, R: Read + 'a>(reader: R) -> ReaderInput<'a> {
        buffered_input_from_reader_with_limit(reader, None).0
    }

    // Helper to collect a few core events for assertions without being fragile
    fn gather_core_events<'a>(mut p: Parser<'a, super::ReaderInput<'a>>) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        for item in &mut p {
            match item {
                Ok((ev, _)) => {
                    // Keep only a small subset we care about
                    match &ev {
                        Event::SequenceStart(_, _)
                        | Event::SequenceEnd
                        | Event::Scalar(..)
                        | Event::StreamStart
                        | Event::StreamEnd
                        | Event::DocumentStart(_)
                        | Event::DocumentEnd => {
                            events.push(ev.clone());
                        }
                        _ => {}
                    }
                }
                Err(_) => break,
            }
        }
        events
    }

    fn collect_scalars_from_reader_bytes(bytes: Vec<u8>) -> Vec<String> {
        let cursor = Cursor::new(bytes);
        let input = buffered_input_from_reader(cursor);
        let parser = Parser::new(input);
        let events = gather_core_events(parser);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::SequenceStart(_, _))),
            "no SequenceStart in events: {:?}",
            events
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::SequenceEnd)),
            "no SequenceEnd in events: {:?}",
            events
        );

        events
            .iter()
            .filter_map(|e| match e {
                Event::Scalar(v, _, _, _) => Some(v.to_string()),
                _ => None,
            })
            .collect()
    }

    fn encode_utf16(text: &str, big_endian: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        let bom = if big_endian {
            0xFEFFu16.to_be_bytes()
        } else {
            0xFEFFu16.to_le_bytes()
        };
        bytes.extend_from_slice(&bom);
        for unit in text.encode_utf16() {
            let encoded = if big_endian {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            };
            bytes.extend_from_slice(&encoded);
        }
        bytes
    }

    #[test]
    fn buffered_input_supports_utf8_and_utf16_reader_inputs() {
        let yaml = "---\n[foo, bar]\n";
        let cases = [
            ("utf-16le with bom", encode_utf16(yaml, false)),
            ("utf-16be with bom", encode_utf16(yaml, true)),
            ("utf-8 without bom", yaml.as_bytes().to_vec()),
            (
                "utf-8 with bom",
                [b"\xEF\xBB\xBF".as_slice(), yaml.as_bytes()].concat(),
            ),
        ];

        for (label, bytes) in cases {
            let scalars = collect_scalars_from_reader_bytes(bytes);
            assert!(
                scalars.contains(&"foo".to_string()) && scalars.contains(&"bar".to_string()),
                "{label}: expected scalar elements 'foo' and 'bar', got {:?}",
                scalars
            );
        }
    }
}
