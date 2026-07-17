//! Streaming, chunked character input helpers.
//!
//! This module provides a small adapter to turn any `std::io::Read` into a
//! streaming iterator of UTF-8 `char`s without loading the entire input into
//! memory. It is used to feed `granit_parser` incrementally, while allowing the
//! upstream `encoding_rs_io` to auto-detect and decode common Unicode encodings
//! (BOM-aware), then buffering via `BufReader`.

use encoding_rs_io::DecodeReaderBytesBuilder;
use granit_parser::{
    BorrowedInput, ErrorKind as ParserErrorKind, FallibleBufferedInput, Input, InputIoError,
};
use std::cell::Cell;
use std::io::{self, BufReader, Read};
use std::rc::Rc;

type DynReader<'a> = Box<dyn Read + 'a>;
type DynBufReader<'a> = BufReader<DynReader<'a>>;

/// Streaming YAML input backed by a reader.
///
/// This does **not** support zero-copy borrowing of scalar slices, so
/// [`BorrowedInput::slice_borrowed`] always returns `None`.
pub struct ReaderInput<'a>(FallibleBufferedInput<ChunkedChars<DynBufReader<'a>>>);

impl<'a> ReaderInput<'a> {
    #[inline]
    pub fn new(inner: FallibleBufferedInput<ChunkedChars<DynBufReader<'a>>>) -> Self {
        Self(inner)
    }
}

impl Input for ReaderInput<'_> {
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

    #[inline]
    fn next_is_z(&self) -> bool {
        self.0.next_is_z()
    }

    #[inline]
    fn take_source_error(&mut self) -> Option<ParserErrorKind> {
        self.0.take_source_error()
    }
}

impl<'a> BorrowedInput<'a> for ReaderInput<'a> {
    #[inline]
    fn slice_borrowed(&self, _start: usize, _end: usize) -> Option<&'a str> {
        None
    }
}

pub(crate) type ReaderInputBytesRead = Rc<Cell<usize>>;

pub struct ChunkedChars<R: Read> {
    /// Optional hard cap on total decoded UTF-8 bytes yielded by this iterator.
    max_bytes: Option<usize>,
    /// Running count of decoded bytes yielded so far across all readers sharing this budget.
    bytes_read: ReaderInputBytesRead,
    /// The underlying reader that already yields UTF-8 bytes (typically a
    /// `BufReader<DecodeReaderBytes<...>>`). It is read incrementally.
    reader: R,
    /// Prevent the source from being polled after EOF or its first error.
    finished: bool,
}

impl<R: Read> ChunkedChars<R> {
    pub fn new(reader: R, max_bytes: Option<usize>, bytes_read: ReaderInputBytesRead) -> Self {
        Self {
            max_bytes,
            bytes_read,
            reader,
            finished: false,
        }
    }
}

impl<R: Read> Iterator for ChunkedChars<R> {
    type Item = Result<char, ParserErrorKind>;

    /// Returns the next Unicode scalar value, a terminal source error, or `None` on clean EOF.
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        // Read exactly one UTF-8 codepoint (1..=4 bytes) from the underlying reader.
        // No internal buffering: rely on the outer BufReader and decoder.
        let mut buf = [0u8; 4];
        let first = loop {
            match self.reader.read(&mut buf[..1]) {
                Ok(0) => {
                    self.finished = true;
                    return None;
                }
                Ok(_) => break buf[0],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    // Retry after interrupted reads.
                }
                Err(error) => {
                    self.finished = true;
                    return Some(Err(ParserErrorKind::InputIo {
                        error: InputIoError::from(error),
                    }));
                }
            }
        };

        let needed = if first < 0x80 {
            1
        } else if first & 0b1110_0000 == 0b1100_0000 {
            2
        } else if first & 0b1111_0000 == 0b1110_0000 {
            3
        } else if first & 0b1111_1000 == 0b1111_0000 {
            4
        } else {
            self.finished = true;
            return Some(Err(ParserErrorKind::InputDecoding {
                message: "invalid UTF-8 leading byte".to_owned(),
            }));
        };

        if needed > 1 {
            let mut read = 0;
            while read < needed - 1 {
                match self.reader.read(&mut buf[1 + read..needed]) {
                    Ok(0) => {
                        self.finished = true;
                        return Some(Err(ParserErrorKind::InputDecoding {
                            message: "unexpected EOF in middle of UTF-8 codepoint".to_owned(),
                        }));
                    }
                    Ok(n) => read += n,
                    Err(e) => {
                        if e.kind() == io::ErrorKind::Interrupted {
                            continue;
                        }
                        self.finished = true;
                        return Some(Err(ParserErrorKind::InputIo {
                            error: InputIoError::from(e),
                        }));
                    }
                }
            }
        }

        let ch = match std::str::from_utf8(&buf[..needed]) {
            Ok(s) => match s.chars().next() {
                Some(ch) => ch,
                None => {
                    self.finished = true;
                    return Some(Err(ParserErrorKind::InputDecoding {
                        message: "decoded UTF-8 codepoint was empty".to_owned(),
                    }));
                }
            },
            Err(error) => {
                self.finished = true;
                return Some(Err(ParserErrorKind::InputDecoding {
                    message: error.to_string(),
                }));
            }
        };

        let total_bytes = self.bytes_read.get();
        if let Some(limit) = self.max_bytes
            && needed > limit.saturating_sub(total_bytes)
        {
            self.finished = true;
            return Some(Err(ParserErrorKind::InputByteLimitExceeded { limit }));
        }
        self.bytes_read.set(total_bytes.saturating_add(needed));

        Some(Ok(ch))
    }
}

/// Creates fallible buffered input and its shared byte counter.
pub fn buffered_input_from_reader_with_limit<'a, R: Read + 'a>(
    reader: R,
    max_bytes: Option<usize>,
) -> (ReaderInput<'a>, ReaderInputBytesRead) {
    let bytes_read: ReaderInputBytesRead = Rc::new(Cell::new(0));
    let input = buffered_input_from_reader_with_limit_shared(reader, max_bytes, bytes_read.clone());
    (input, bytes_read)
}

/// Like [`buffered_input_from_reader_with_limit`] but uses an existing shared byte counter.
pub fn buffered_input_from_reader_with_limit_shared<'a, R: Read + 'a>(
    reader: R,
    max_bytes: Option<usize>,
    bytes_read: ReaderInputBytesRead,
) -> ReaderInput<'a> {
    // Auto-detect encoding (BOM or guess), decode to UTF-8 on the fly.
    let decoder = DecodeReaderBytesBuilder::new()
        .encoding(None) // None = sniff BOM / use heuristics; set Some(encoding) to force
        .bom_override(true)
        .build(reader);

    let br = BufReader::new(Box::new(decoder) as DynReader<'a>);
    let char_iter = ChunkedChars::new(br, max_bytes, bytes_read);
    ReaderInput::new(FallibleBufferedInput::new(char_iter))
}

#[cfg(test)]
mod tests {
    use crate::buffered_input::ChunkedChars;
    use crate::buffered_input::ReaderInput;
    use crate::buffered_input::buffered_input_from_reader_with_limit;
    use granit_parser::{Event, Parser};
    use std::cell::Cell;
    use std::io::{Cursor, Read};
    use std::rc::Rc;

    struct InterruptOnceReader {
        bytes: Vec<u8>,
        pos: usize,
        interrupt_at: usize,
        interrupted: bool,
    }

    impl InterruptOnceReader {
        fn new(bytes: Vec<u8>, interrupt_at: usize) -> Self {
            Self {
                bytes,
                pos: 0,
                interrupt_at,
                interrupted: false,
            }
        }
    }

    impl Read for InterruptOnceReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos == self.interrupt_at && !self.interrupted {
                self.interrupted = true;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "interrupted once",
                ));
            }
            if buf.is_empty() || self.pos >= self.bytes.len() {
                return Ok(0);
            }

            buf[0] = self.bytes[self.pos];
            self.pos += 1;
            Ok(1)
        }
    }

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
                        Event::SequenceStart(_, _, _)
                        | Event::SequenceEnd
                        | Event::Scalar(..)
                        | Event::StreamStart
                        | Event::StreamEnd
                        | Event::DocumentStart(..)
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
                .any(|e| matches!(e, Event::SequenceStart(_, _, _))),
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

    #[test]
    fn chunked_chars_retries_interrupted_first_byte_reads() {
        let bytes_read = Rc::new(Cell::new(0));
        let reader = InterruptOnceReader::new(b"a".to_vec(), 0);
        let mut chars = ChunkedChars::new(reader, None, bytes_read);

        assert_eq!(chars.next(), Some(Ok('a')));
    }

    #[test]
    fn chunked_chars_retries_interrupted_continuation_reads() {
        let bytes_read = Rc::new(Cell::new(0));
        let reader = InterruptOnceReader::new("é".as_bytes().to_vec(), 1);
        let mut chars = ChunkedChars::new(reader, None, bytes_read);

        assert_eq!(chars.next(), Some(Ok('é')));
    }
}
