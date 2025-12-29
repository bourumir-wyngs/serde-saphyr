//! Streaming, chunked character input helpers.
//!
//! This module provides a small adapter to turn any `std::io::Read` into a
//! streaming iterator of UTF-8 `char`s without loading the entire input into
//! memory. It is used to feed `saphyr_parser` incrementally, while allowing the
//! upstream `encoding_rs_io` to auto-detect and decode common Unicode encodings
//! (BOM-aware), then buffering via `BufReader`.

use encoding_rs_io::DecodeReaderBytesBuilder;
use saphyr_parser::BufferedInput;
use std::cell::RefCell;
use std::io::{self, BufReader, Error, Read};
use std::rc::Rc;

type DynReader<'a> = Box<dyn Read + 'a>;
type DynBufReader<'a> = BufReader<DynReader<'a>>;
pub type ReaderInput<'a> = BufferedInput<ChunkedChars<DynBufReader<'a>>>;
pub type ReaderInputError = Rc<RefCell<Option<Error>>>;

pub struct ChunkedChars<R: Read> {
    /// Optional hard cap on total decoded UTF-8 bytes yielded by this iterator.
    max_bytes: Option<usize>,
    /// Running count of decoded bytes yielded so far (from the underlying reader).
    total_bytes: usize,
    /// The underlying reader that already yields UTF-8 bytes (typically a
    /// `BufReader<DecodeReaderBytes<...>>`). It is read incrementally.
    reader: R,
    /// Remember IO error, if any, here to report it later. This must be shared,
    /// as otherwise we cannot later reach with Saphyr parser API
    pub(crate) err: Rc<RefCell<Option<Error>>>,
}

impl<R: Read> ChunkedChars<R> {
    pub fn new(reader: R, max_bytes: Option<usize>, err: Rc<RefCell<Option<Error>>>) -> Self {
        Self {
            max_bytes,
            total_bytes: 0,
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
        if let Some(limit) = self.max_bytes {
            let new_total = self.total_bytes.saturating_add(add);
            if new_total > limit {
                self.err.replace(Some(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    format!("input size limit of {limit} bytes exceeded"),
                )));
                return None;
            }
            self.total_bytes = new_total;
        } else {
            self.total_bytes = self.total_bytes.saturating_add(add);
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
) -> (ReaderInput<'a>, ReaderInputError) {
    // Auto-detect encoding (BOM or guess), decode to UTF-8 on the fly.
    let decoder = DecodeReaderBytesBuilder::new()
        .encoding(None) // None = sniff BOM / use heuristics; set Some(encoding) to force
        .build(reader);

    let error: ReaderInputError = Rc::new(RefCell::new(None));

    let br = BufReader::new(Box::new(decoder) as DynReader<'a>);
    let char_iter = ChunkedChars::new(br, max_bytes, error.clone());

    (BufferedInput::new(char_iter), error)
}

#[cfg(test)]
mod tests {
    use crate::buffered_input::{ChunkedChars, buffered_input_from_reader_with_limit};
    use saphyr_parser::{BufferedInput, Event, Parser};
    use std::io::{BufReader, Cursor, Read};

    pub fn buffered_input_from_reader<'a, R: Read + 'a>(
        reader: R,
    ) -> BufferedInput<ChunkedChars<BufReader<Box<dyn Read + 'a>>>> {
        buffered_input_from_reader_with_limit(reader, None).0
    }

    // Helper to collect a few core events for assertions without being fragile
    fn gather_core_events(
        mut p: Parser<
            'static,
            BufferedInput<super::ChunkedChars<std::io::BufReader<Box<dyn std::io::Read>>>>,
        >,
    ) -> Vec<Event<'static>> {
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

    #[test]
    fn buffered_input_handles_utf16le_bom() {
        // YAML: "---\n[1, 2]\n"
        // Encode as UTF-16LE with BOM (FF FE)
        let code_units: [u16; 9] = [
            0xFEFF, // BOM (when written as u16 then to LE bytes becomes FF FE at start)
            '-' as u16,
            '-' as u16,
            '-' as u16,
            '\n' as u16,
            '[' as u16,
            '1' as u16,
            ',' as u16,
            ' ' as u16,
        ];
        let mut bytes = Vec::new();
        for &cu in &code_units {
            bytes.extend_from_slice(&cu.to_le_bytes());
        }
        // Continue the rest of the string: "2]\n"
        for ch in ['2', ']', '\n'] {
            bytes.extend_from_slice(&(ch as u16).to_le_bytes());
        }

        let cursor = Cursor::new(bytes);
        let input = buffered_input_from_reader(cursor);
        let parser = Parser::new(input);
        let events = gather_core_events(parser);

        // Expect a sequence with two scalar elements "1" and "2"
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

        let scalars: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                Event::Scalar(v, _, _, _) => Some(v.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            scalars.contains(&"1".to_string()) && scalars.contains(&"2".to_string()),
            "expected scalar elements '1' and '2', got {:?}",
            scalars
        );
    }

    #[test]
    fn buffered_input_handles_utf8_basic() {
        let yaml = "---\n[foo, bar]\n";
        let cursor = Cursor::new(yaml.as_bytes());
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
        let scalars: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                Event::Scalar(v, _, _, _) => Some(v.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            scalars.contains(&"foo".to_string()) && scalars.contains(&"bar".to_string()),
            "expected scalar elements 'foo' and 'bar', got {:?}",
            scalars
        );
    }
}
