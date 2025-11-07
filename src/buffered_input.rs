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

/// IO error replacement char
pub(crate) const IO_ERROR_CHAR: char = '\u{FFFD}';

pub struct ChunkedChars<R: Read> {
    /// Optional hard cap on total decoded UTF-8 bytes yielded by this iterator.
    max_bytes: Option<usize>,
    /// Running count of decoded bytes yielded so far (from the underlying reader).
    total_bytes: usize,
    /// The underlying reader that already yields UTF-8 bytes (typically a
    /// `BufReader<DecodeReaderBytes<...>>`). It is read incrementally.
    reader: R,
    /// Buffer holding the currently available decoded UTF-8 slice that hasn't
    /// been fully iterated yet.
    buf: String,
    /// Byte index into `buf` pointing at the start of the next character to
    /// return from the iterator.
    idx: usize,
    /// Reusable temporary byte buffer used to read the next chunk from
    /// `reader` before converting it into `buf`.
    tmp: Vec<u8>,
    /// Remember IO error, if any, here to report it later. This must be shared,
    /// as otherwise we cannot later reach with Saphyr parser API
    pub(crate) err: Rc<RefCell<Option<Error>>>,
}

impl<R: Read> ChunkedChars<R> {
    pub fn new(
        reader: R,
        max_bytes: Option<usize>,
        err: Rc<RefCell<Option<Error>>>,
    ) -> Self {
        Self {
            max_bytes,
            total_bytes: 0,
            reader,
            buf: String::new(),
            idx: 0,
            tmp: vec![0u8; 8 * 1024],
            err,
        }
    }

    /// Refill `buf` with the next chunk of decoded UTF-8.
    ///
    /// Returns `Ok(true)` when new data has been loaded into `buf`,
    /// `Ok(false)` on EOF, or an `Err` if the underlying reader
    /// returned an I/O error.
    #[inline]
    fn refill(&mut self) -> io::Result<bool> {
        // Defensive: cap the number of consecutive iterations that do not
        // make progress (should never happen with a well-behaved reader/decoder).
        let mut spin_guards = 0;
        loop {
            let n = self.reader.read(&mut self.tmp)?;
            if n == 0 {
                return Ok(false); // EOF
            }
            // Safe: reader is guaranteed to output valid UTF-8.
            let s = std::str::from_utf8(&self.tmp[..n])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if !s.is_empty() {
                // Enforce byte limit on decoded bytes if configured
                if let Some(limit) = self.max_bytes {
                    self.total_bytes = self.total_bytes.saturating_add(s.len());
                    if self.total_bytes > limit {
                        return Err(io::Error::new(
                            io::ErrorKind::FileTooLarge,
                            format!("input size limit of {limit} bytes exceeded"),
                        ));
                    }
                }
                self.buf.clear();
                self.buf.push_str(s);
                self.idx = 0;
                return Ok(true);
            }
            // If we somehow got empty (can only happen with buggy Read), try again a few times
            // then stop.
            spin_guards += 1;
            if spin_guards > 128 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "read keeps returning Ok(0) 128 times without EOF",
                ));
            }
        }
    }
}

impl<R: Read> Iterator for ChunkedChars<R> {
    type Item = char;

    /// Returns the next Unicode scalar value from the stream, or `None` on EOF.
    /// If error occurs, sets the error field that is a shared reference to the
    /// error value, so that the parser can later pick this up.
    fn next(&mut self) -> Option<char> {
        loop {
            if self.idx < self.buf.len() {
                // Read one char and advance by its UTF-8 byte width.
                let ch = self.buf[self.idx..].chars().next().unwrap_or(IO_ERROR_CHAR);
                self.idx += ch.len_utf8();
                return Some(ch);
            }
            // Need more data.
            match self.refill() {
                Ok(true) => continue,
                Ok(false) => return None, // EOF
                Err(error) => {
                    self.err.replace(Some(error));
                    return None; // Return EOF, err will be checked later.
                }
            }
        }
    }
}

/// Creates buffered input and returns both input and reference to the variable
/// holding the possible error. We cannot otherwise later reach our ChunkedChars.
pub fn buffered_input_from_reader_with_limit<'a, R: Read + 'a>(
    reader: R,
    max_bytes: Option<usize>,
) -> (
    BufferedInput<ChunkedChars<BufReader<Box<dyn Read + 'a>>>>,
    Rc<RefCell<Option<Error>>>,
) {
    // Auto-detect encoding (BOM or guess), decode to UTF-8 on the fly.
    let decoder = DecodeReaderBytesBuilder::new()
        .encoding(None) // None = sniff BOM / use heuristics; set Some(encoding) to force
        .build(reader);

    let error: Rc<RefCell<Option<Error>>> = Rc::new(RefCell::new(None));

    let br = BufReader::new(Box::new(decoder) as Box<dyn Read + 'a>);
    let char_iter = ChunkedChars::new(br, max_bytes, error.clone());

    (BufferedInput::new(char_iter), error)
}

#[cfg(test)]
mod tests {
    use crate::buffered_input::{buffered_input_from_reader_with_limit, ChunkedChars};
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
