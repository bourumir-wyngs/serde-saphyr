use std::cell::RefCell;
use std::io::{self, Read};
use std::rc::Rc;

/// Assumed line length to estimate the size of buffers.
/// This is more than YAML typically contains.
const ASSUMED_LINE_LENGTH: usize = 512;

/// How many most-recent bytes to retain for diagnostics/snippet rendering.
///
/// Pick this >> MAX_READ_AHEAD so read-ahead doesn't evict all "history".
pub(crate) const RING_BUFFER_SIZE: usize = 6 * ASSUMED_LINE_LENGTH;

/// Maximum number of bytes we are allowed to read-ahead beyond what the consumer has read.
/// This read-ahead happens ONLY inside `get_recent()`.
pub(crate) const MAX_READ_AHEAD: usize = 2 * ASSUMED_LINE_LENGTH;

// -------------------------
// Fixed-size ring buffer
// -------------------------

/// A fixed-size ring buffer backed by an array.
///
/// This replaces `VecDeque<u8>` for cases where the maximum size is known at compile time.
/// Uses a circular buffer with head and tail indices.
struct FixedRingBuffer<const N: usize> {
    /// The underlying storage array.
    data: [u8; N],
    /// Index of the first (oldest) element. Valid when count > 0.
    head: usize,
    /// Number of elements currently in the buffer.
    count: usize,
}

impl<const N: usize> FixedRingBuffer<N> {
    /// Create a new empty ring buffer.
    const fn new() -> Self {
        Self {
            data: [0u8; N],
            head: 0,
            count: 0,
        }
    }

    /// Returns true if the buffer is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns the number of elements in the buffer.
    #[inline]
    fn len(&self) -> usize {
        self.count
    }

    /// Push a byte to the back of the buffer.
    /// If the buffer is full, the oldest element is overwritten.
    #[inline]
    fn push_back(&mut self, value: u8) {
        let tail = (self.head + self.count) % N;
        self.data[tail] = value;
        if self.count == N {
            // Buffer is full, advance head (overwrite oldest)
            self.head = (self.head + 1) % N;
        } else {
            self.count += 1;
        }
    }

    /// Remove and return the front (oldest) element, or None if empty.
    #[inline]
    fn pop_front(&mut self) -> Option<u8> {
        if self.count == 0 {
            None
        } else {
            let value = self.data[self.head];
            self.head = (self.head + 1) % N;
            self.count -= 1;
            Some(value)
        }
    }

    /// Returns an iterator over the elements from oldest to newest.
    fn iter(&self) -> FixedRingBufferIter<'_, N> {
        FixedRingBufferIter {
            buffer: self,
            pos: 0,
        }
    }
}

impl<const N: usize> std::fmt::Debug for FixedRingBuffer<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixedRingBuffer")
            .field("head", &self.head)
            .field("count", &self.count)
            .field("capacity", &N)
            .finish()
    }
}

/// Iterator over a `FixedRingBuffer`.
struct FixedRingBufferIter<'a, const N: usize> {
    buffer: &'a FixedRingBuffer<N>,
    pos: usize,
}

impl<'a, const N: usize> Iterator for FixedRingBufferIter<'a, N> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buffer.count {
            None
        } else {
            let idx = (self.buffer.head + self.pos) % N;
            self.pos += 1;
            Some(self.buffer.data[idx])
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buffer.count - self.pos;
        (remaining, Some(remaining))
    }
}

impl<'a, const N: usize> ExactSizeIterator for FixedRingBufferIter<'a, N> {}

/// A snapshot of bytes currently retained by the reader wrapper.
///
/// Offsets are absolute byte offsets from the start of the underlying stream.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used in tests
pub(crate) struct RecentSnapshot {
    /// Absolute byte offset of `bytes[0]`.
    pub start_offset: u64,
    /// Absolute byte offset one-past-the-end of `bytes`.
    pub end_offset: u64,
    /// 1-based line number at `bytes[0]}`.
    pub start_line: usize,
    /// Snapshot bytes (oldest -> newest).
    pub bytes: Vec<u8>,
}

/// A `Read` wrapper that:
/// - retains the last `RING_BUFFER_SIZE` bytes in a ring buffer
/// - tracks the absolute byte offset returned to the consumer
/// - allows bounded read-ahead ONLY via `get_recent()`
///
/// Key invariants:
/// - `offset()` is the absolute offset after bytes returned to the consumer.
/// - `stash` contains bytes read from `inner` but not yet returned.
/// - Therefore the next `inner.read()` begins at `offset() + stash.len()`.
pub(crate) struct RingReader<R> {
    inner: R,

    // Ring buffer of the most-recent bytes (includes both returned bytes and read-ahead bytes).
    ring: FixedRingBuffer<RING_BUFFER_SIZE>,
    // Absolute offset of ring[0] (valid only when ring is non-empty).
    ring_start_offset: u64,
    // 1-based line number at ring[0] (valid only when ring is non-empty).
    ring_start_line: usize,

    // Read-ahead bytes (only filled by get_recent()).
    //
    // Why is `stash` a struct field rather than a local variable in `get_recent()`?
    //
    // The stash holds bytes that have been read from `inner` during `get_recent()` but have
    // NOT yet been returned to the consumer. This design enables **continued use of the reader
    // after calling `get_recent()`**: the consumer can inspect the snapshot for error diagnostics
    // and then resume reading from where they left off without losing any bytes.
    //
    // When `read()` is called, it first drains bytes from the stash before reading from `inner`
    // again. This preserves stream integrity - the consumer sees the exact same byte sequence
    // as if no read-ahead had occurred.
    //
    // If `stash` were a local variable in `get_recent()`, the read-ahead bytes would be lost
    // when the method returns, and subsequent `read()` calls would skip those bytes entirely,
    // corrupting the byte stream from the consumer's perspective.
    stash: FixedRingBuffer<MAX_READ_AHEAD>,

    // Absolute offset after bytes returned to the consumer.
    returned_total: u64,
}

impl<R> RingReader<R> {
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            ring: FixedRingBuffer::new(),
            ring_start_offset: 0,
            ring_start_line: 1,
            stash: FixedRingBuffer::new(),
            returned_total: 0,
        }
    }

    /// Absolute offset after bytes already returned to the consumer.
    #[allow(dead_code)] // Used in tests
    pub(crate) fn offset(&self) -> u64 {
        self.returned_total
    }

    /// How many bytes are currently read-ahead (not yet returned to the consumer).
    #[allow(dead_code)] // Used in tests
    pub(crate) fn read_ahead_len(&self) -> usize {
        self.stash.len()
    }

    #[allow(dead_code)] // Used in tests
    pub(crate) fn inner(&self) -> &R {
        &self.inner
    }

    #[allow(dead_code)] // Used in tests
    pub(crate) fn inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    #[allow(dead_code)] // Used in tests
    pub(crate) fn into_inner(self) -> R {
        self.inner
    }

    /// Get a snapshot of the most recently retained bytes and their absolute offset range.
    ///
    /// Behavior:
    /// - This is the ONLY method that may read-ahead from `inner`.
    /// - It will read at most `MAX_READ_AHEAD - stash.len()` additional bytes (never more).
    /// - The returned snapshot is trimmed to "reasonable UTF-8 boundaries" at the edges
    ///   (leading continuation bytes are dropped; incomplete trailing sequence is dropped).
    ///
    /// Returns:
    /// - `Ok(RecentSnapshot)` on success (even if EOF => snapshot may be empty)
    /// - `Err(io::Error)` if the underlying reader errors during read-ahead
    pub(crate) fn get_recent(&mut self) -> io::Result<RecentSnapshot>
    where
        R: Read,
    {
        // Enforce the GLOBAL read-ahead cap: total unread-ahead bytes must not exceed MAX_READ_AHEAD.
        let already_ahead = self.stash.len();
        let can_read_more = MAX_READ_AHEAD.saturating_sub(already_ahead);

        if can_read_more > 0 {
            let _ = self.read_ahead_at_most(can_read_more)?;
        }

        let (mut start_offset, mut start_line, mut bytes) = self.ring_snapshot();
        if !bytes.is_empty() {
            (start_offset, start_line, bytes) =
                trim_to_utf8_boundaries_with_line(bytes, start_offset, start_line);
        }

        let end_offset = start_offset.saturating_add(bytes.len() as u64);

        Ok(RecentSnapshot {
            start_offset,
            end_offset,
            start_line,
            bytes,
        })
    }

    fn next_inner_offset(&self) -> u64 {
        // Next unread position in the underlying stream is after returned bytes + unread stash bytes.
        self.returned_total.saturating_add(self.stash.len() as u64)
    }

    /// Returns (start_offset, start_line, bytes).
    fn ring_snapshot(&self) -> (u64, usize, Vec<u8>) {
        if self.ring.is_empty() {
            // No bytes retained; represent an empty range at the current consumer offset.
            return (self.returned_total, self.ring_start_line, Vec::new());
        }
        let start_offset = self.ring_start_offset;
        let start_line = self.ring_start_line;
        let bytes: Vec<u8> = self.ring.iter().collect();
        (start_offset, start_line, bytes)
    }

    pub(crate) fn push_ring_bytes(&mut self, bytes: &[u8], abs_start: u64) {
        let mut off = abs_start;

        for &b in bytes {
            if self.ring.is_empty() {
                self.ring_start_offset = off;
            }

            if self.ring.len() == RING_BUFFER_SIZE {
                let evicted = self.ring.pop_front();
                self.ring_start_offset = self.ring_start_offset.saturating_add(1);
                // Track newlines: if we evict a newline, increment the start line
                if evicted == Some(b'\n') {
                    self.ring_start_line = self.ring_start_line.saturating_add(1);
                }
            }

            self.ring.push_back(b);
            off = off.saturating_add(1);
        }
    }

    fn read_ahead_at_most(&mut self, max_additional: usize) -> io::Result<usize>
    where
        R: Read,
    {
        if max_additional == 0 {
            return Ok(0);
        }

        // Small fixed scratch buffer to avoid heap churn.
        const SCRATCH: usize = 8 * 1024;
        let mut scratch = [0u8; SCRATCH];

        let mut remaining = max_additional;
        let mut total = 0usize;

        while remaining > 0 {
            let want = remaining.min(SCRATCH);
            let n = self.inner.read(&mut scratch[..want])?;
            if n == 0 {
                break; // EOF
            }

            // Absolute offset of the first newly read byte.
            let abs_start = self.next_inner_offset();
            let chunk = &scratch[..n];

            // Stash it so the consumer still sees the same byte stream.
            for &b in chunk {
                self.stash.push_back(b);
            }

            // Also record into ring for snapshot context.
            self.push_ring_bytes(chunk, abs_start);

            total = total.saturating_add(n);
            remaining = remaining.saturating_sub(n);
        }

        Ok(total)
    }

    fn drain_stash_into(&mut self, out: &mut [u8]) -> usize {
        let mut n = 0usize;

        while n < out.len() {
            let b = match self.stash.pop_front() {
                Some(x) => x,
                None => break,
            };
            out[n] = b;
            n = n.saturating_add(1);
        }

        n
    }
}

impl<R: Read> Read for RingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Always serve read-ahead bytes first.
        if !self.stash.is_empty() {
            let n = self.drain_stash_into(buf);
            self.returned_total = self.returned_total.saturating_add(n as u64);
            return Ok(n);
        }

        // No read-ahead pending: read exactly what the consumer asks for.
        let n = self.inner.read(buf)?;
        if n == 0 {
            return Ok(0);
        }

        let chunk = match buf.get(..n) {
            Some(s) => s,
            None => return Ok(0), // defensive: should be impossible
        };

        let abs_start = self.returned_total; // stash empty => next_inner_offset == returned_total
        self.push_ring_bytes(chunk, abs_start);

        self.returned_total = self.returned_total.saturating_add(n as u64);
        Ok(n)
    }
}

// -------------------------
// Shared RingReader for use in lib.rs
// -------------------------

/// A shared wrapper around `RingReader` that allows multiple references.
///
/// This is used in `lib.rs` to wrap the reader before passing to `LiveEvents`,
/// while retaining access to get snapshots when errors occur.
pub(crate) struct SharedRingReader<R> {
    inner: Rc<RefCell<RingReader<R>>>,
}

impl<R> SharedRingReader<R> {
    /// Create a new shared ring reader.
    pub(crate) fn new(reader: R) -> Self {
        Self {
            inner: Rc::new(RefCell::new(RingReader::new(reader))),
        }
    }

    /// Get a snapshot of the recent bytes for error reporting.
    pub(crate) fn get_recent(&self) -> io::Result<RecentSnapshot>
    where
        R: Read,
    {
        self.inner.borrow_mut().get_recent()
    }

    /// Clone the inner Rc for sharing.
    pub(crate) fn clone_inner(&self) -> Rc<RefCell<RingReader<R>>> {
        Rc::clone(&self.inner)
    }
}

/// A reader handle that delegates to a shared `RingReader`.
///
/// This implements `Read` and can be passed to functions that consume readers,
/// while the original `SharedRingReader` retains access for snapshots.
pub(crate) struct SharedRingReaderHandle<R> {
    inner: Rc<RefCell<RingReader<R>>>,
}

impl<R> SharedRingReaderHandle<R> {
    pub(crate) fn new(shared: &SharedRingReader<R>) -> Self {
        Self {
            inner: shared.clone_inner(),
        }
    }
}

impl<R: Read> Read for SharedRingReaderHandle<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.borrow_mut().read(buf)
    }
}

// -------------------------
// UTF-8 edge trimming helpers
// -------------------------

fn is_utf8_continuation(b: u8) -> bool {
    (b & 0b1100_0000) == 0b1000_0000
}

/// Return expected UTF-8 sequence length for valid lead bytes.
/// Returns None if `b` is not a valid lead byte.
fn utf8_expected_len(lead: u8) -> Option<usize> {
    if lead <= 0x7F {
        Some(1)
    } else if (0xC2..=0xDF).contains(&lead) {
        Some(2)
    } else if (0xE0..=0xEF).contains(&lead) {
        Some(3)
    } else if (0xF0..=0xF4).contains(&lead) {
        Some(4)
    } else {
        None
    }
}

/// Trim to "reasonable UTF-8 boundaries" and also track line numbers:
/// - drop leading continuation bytes (and adjust start_offset and start_line accordingly)
/// - drop a trailing incomplete UTF-8 sequence (if snapshot ends mid-codepoint)
fn trim_to_utf8_boundaries_with_line(
    mut bytes: Vec<u8>,
    mut start_offset: u64,
    mut start_line: usize,
) -> (u64, usize, Vec<u8>) {
    if bytes.is_empty() {
        return (start_offset, start_line, bytes);
    }

    // 1) Trim leading continuation bytes, counting newlines in the trimmed portion.
    let mut cut = 0usize;
    while cut < bytes.len() && is_utf8_continuation(bytes[cut]) {
        // Note: '\n' (0x0A) is not a continuation byte, so we won't see it here.
        // But for safety, check anyway.
        if bytes[cut] == b'\n' {
            start_line = start_line.saturating_add(1);
        }
        cut = cut.saturating_add(1);
    }
    if cut > 0 {
        bytes.drain(..cut);
        start_offset = start_offset.saturating_add(cut as u64);
    }

    // 2) Trim trailing incomplete sequence.
    trim_incomplete_utf8_tail(&mut bytes);

    (start_offset, start_line, bytes)
}

/// Trim to "reasonable UTF-8 boundaries" without panicking and without mutating interior bytes:
/// - drop leading continuation bytes (and adjust start_offset accordingly)
/// - drop a trailing incomplete UTF-8 sequence (if snapshot ends mid-codepoint)
fn trim_incomplete_utf8_tail(bytes: &mut Vec<u8>) {
    loop {
        if bytes.is_empty() {
            return;
        }

        // Count trailing continuation bytes (0..=3).
        let mut cont = 0usize;
        let mut i = bytes.len();
        while i > 0 && cont < 3 {
            let b = bytes[i - 1];
            if is_utf8_continuation(b) {
                cont = cont.saturating_add(1);
                i = i.saturating_sub(1);
            } else {
                break;
            }
        }

        if i == 0 {
            // All bytes are continuation bytes (invalid boundary); drop all.
            bytes.clear();
            return;
        }

        let lead_idx = i - 1;
        let lead = bytes[lead_idx];

        let expected = match utf8_expected_len(lead) {
            Some(n) => n,
            None => {
                // Not a valid lead byte => we won't try to "fix" it here.
                // Consider it "reasonable enough" (caller can decode lossy).
                return;
            }
        };

        let actual = bytes.len().saturating_sub(lead_idx);

        if actual < expected {
            // Incomplete codepoint at end: drop from lead byte onwards.
            bytes.truncate(lead_idx);
            // New end might still be incomplete (rare), so loop.
            continue;
        }

        // End is not "truncated mid-codepoint".
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read};

    /// A deterministic reader for tests:
    /// - serves bytes from an internal buffer
    /// - counts how many bytes have been read from it
    /// - can limit max chunk size per read() call (to force partial reads)
    #[derive(Debug)]
    struct CountingReader {
        data: Vec<u8>,
        pos: usize,
        bytes_read_total: usize,
        max_chunk: usize,
    }

    impl CountingReader {
        fn new(data: Vec<u8>) -> Self {
            Self::with_max_chunk(data, usize::MAX)
        }

        fn with_max_chunk(data: Vec<u8>, max_chunk: usize) -> Self {
            Self {
                data,
                pos: 0,
                bytes_read_total: 0,
                max_chunk: max_chunk.max(1),
            }
        }

        fn bytes_read(&self) -> usize {
            self.bytes_read_total
        }
    }

    impl Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            if self.pos >= self.data.len() {
                return Ok(0);
            }

            let remaining = self.data.len().saturating_sub(self.pos);
            let n = buf.len().min(self.max_chunk).min(remaining);

            // Defensive (should never be 0 here unless buf.len()==0 or remaining==0).
            if n == 0 {
                return Ok(0);
            }

            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            self.bytes_read_total += n;
            Ok(n)
        }
    }

    fn make_ascii_data(len: usize) -> Vec<u8> {
        (0..len).map(|i| b'a' + (i % 26) as u8).collect()
    }

    fn assert_snapshot_offsets_consistent(s: &RecentSnapshot) {
        assert_eq!(s.end_offset, s.start_offset + s.bytes.len() as u64);
    }

    #[test]
    fn empty_input_snapshot_is_empty_and_offsets_zero() {
        let inner = CountingReader::new(Vec::new());
        let mut rr = RingReader::new(inner);

        assert_eq!(rr.offset(), 0);
        assert_eq!(rr.read_ahead_len(), 0);

        let snap = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap);

        assert_eq!(snap.start_offset, 0);
        assert_eq!(snap.end_offset, 0);
        assert!(snap.bytes.is_empty());

        assert_eq!(rr.offset(), 0);
        assert_eq!(rr.read_ahead_len(), 0);
        assert_eq!(rr.inner().bytes_read(), 0);
    }

    #[test]
    fn read_with_empty_buffer_returns_0_and_does_not_touch_inner() {
        let data = make_ascii_data(128);
        let inner = CountingReader::new(data);
        let mut rr = RingReader::new(inner);

        let mut buf = [0u8; 0];
        let n = rr.read(&mut buf).unwrap();
        assert_eq!(n, 0);

        assert_eq!(rr.offset(), 0);
        assert_eq!(rr.read_ahead_len(), 0);
        assert_eq!(rr.inner().bytes_read(), 0);
    }

    #[test]
    fn get_recent_reads_ahead_up_to_max_and_does_not_advance_offset() {
        let data = make_ascii_data(MAX_READ_AHEAD * 3 + 123);
        let inner = CountingReader::with_max_chunk(data.clone(), 7);
        let mut rr = RingReader::new(inner);

        assert_eq!(rr.offset(), 0);
        assert_eq!(rr.read_ahead_len(), 0);

        let snap = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap);

        // Read-ahead happens, but consumer offset must not change.
        assert_eq!(rr.offset(), 0);

        // Stash should be filled to the cap (data is long enough).
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_eq!(rr.inner().bytes_read(), MAX_READ_AHEAD);

        // Ring should contain the read-ahead bytes (ASCII => no UTF trimming).
        assert_eq!(snap.start_offset, 0);
        assert_eq!(snap.bytes.len(), MAX_READ_AHEAD);
        assert_eq!(&snap.bytes[..], &data[..MAX_READ_AHEAD]);
    }

    #[test]
    fn get_recent_does_not_read_more_when_stash_already_full() {
        let data = make_ascii_data(MAX_READ_AHEAD * 2 + 5);
        let inner = CountingReader::with_max_chunk(data.clone(), 5);
        let mut rr = RingReader::new(inner);

        let snap1 = rr.get_recent().unwrap();
        let inner_read1 = rr.inner().bytes_read();
        assert_eq!(inner_read1, MAX_READ_AHEAD);
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_snapshot_offsets_consistent(&snap1);

        let snap2 = rr.get_recent().unwrap();
        let inner_read2 = rr.inner().bytes_read();
        assert_eq!(inner_read2, inner_read1, "inner must not be read again");
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_snapshot_offsets_consistent(&snap2);

        assert_eq!(snap2.bytes, snap1.bytes);
        assert_eq!(snap2.start_offset, snap1.start_offset);
        assert_eq!(snap2.end_offset, snap1.end_offset);
    }

    #[test]
    fn read_drains_stash_before_touching_inner_again() {
        let data = make_ascii_data(MAX_READ_AHEAD * 2 + 77);
        let inner = CountingReader::with_max_chunk(data.clone(), 11);
        let mut rr = RingReader::new(inner);

        rr.get_recent().unwrap();
        assert_eq!(rr.offset(), 0);
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);

        let inner_before = rr.inner().bytes_read();

        let mut first = vec![0u8; 17];
        rr.read_exact(&mut first).unwrap();

        assert_eq!(&first[..], &data[..17]);
        assert_eq!(rr.offset(), 17);
        assert_eq!(
            rr.inner().bytes_read(),
            inner_before,
            "must not read inner while draining stash"
        );
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD - 17);
    }

    #[test]
    fn get_recent_refills_only_the_missing_amount_to_reach_cap() {
        let data = make_ascii_data(MAX_READ_AHEAD * 3 + 10);
        let inner = CountingReader::with_max_chunk(data.clone(), 3);
        let mut rr = RingReader::new(inner);

        rr.get_recent().unwrap();
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_eq!(rr.inner().bytes_read(), MAX_READ_AHEAD);

        let drain = 100usize.min(MAX_READ_AHEAD);
        let mut tmp = vec![0u8; drain];
        rr.read_exact(&mut tmp).unwrap();
        assert_eq!(&tmp[..], &data[..drain]);

        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD - drain);

        let inner_before = rr.inner().bytes_read();
        rr.get_recent().unwrap();
        let inner_after = rr.inner().bytes_read();

        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_eq!(
            inner_after - inner_before,
            drain,
            "must read only the missing bytes"
        );
    }

    #[test]
    fn stream_integrity_with_read_ahead_matches_original_input() {
        let data = make_ascii_data(MAX_READ_AHEAD * 2 + 123);
        let inner = CountingReader::with_max_chunk(data.clone(), 9);
        let mut rr = RingReader::new(inner);

        // Trigger read-ahead
        rr.get_recent().unwrap();
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_eq!(rr.offset(), 0);

        // Read entire stream and compare
        let mut out = Vec::new();
        rr.read_to_end(&mut out).unwrap();

        assert_eq!(out, data);
        assert_eq!(rr.offset(), data.len() as u64);
        assert_eq!(rr.read_ahead_len(), 0);
        assert_eq!(rr.inner().bytes_read(), data.len());
    }

    #[test]
    fn stream_integrity_with_periodic_snapshots_matches_original_input() {
        let data = make_ascii_data(MAX_READ_AHEAD * 3 + 2000);
        let inner = CountingReader::with_max_chunk(data.clone(), 17);
        let mut rr = RingReader::new(inner);

        let mut out = Vec::new();
        let mut buf = vec![0u8; 57];

        // Keep reading; occasionally request snapshots (which may read-ahead).
        while out.len() < data.len() {
            // Snapshot every ~400 bytes produced (best-effort; does not need to align).
            if out.len() % 400 == 0 {
                let snap = rr.get_recent().unwrap();
                assert_snapshot_offsets_consistent(&snap);
                // At any moment, unread read-ahead must never exceed cap.
                assert!(rr.read_ahead_len() <= MAX_READ_AHEAD);
            }

            let n = rr.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }

        assert_eq!(out, data);
        assert_eq!(rr.offset(), data.len() as u64);
        assert_eq!(rr.inner().bytes_read(), data.len());
    }

    #[test]
    fn ring_eviction_reports_correct_window_and_offset() {
        let extra = 123usize;
        let total = RING_BUFFER_SIZE + extra;
        let data = make_ascii_data(total);

        let inner = std::io::Cursor::new(data.clone());
        let mut rr = RingReader::new(inner);

        let mut out = Vec::new();
        rr.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
        assert_eq!(rr.offset(), total as u64);

        let snap = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap);

        assert_eq!(snap.bytes.len(), RING_BUFFER_SIZE);
        assert_eq!(snap.start_offset, extra as u64);
        assert_eq!(snap.end_offset, total as u64);
        assert_eq!(&snap.bytes[..], &data[extra..]);
    }

    #[test]
    fn get_recent_trims_leading_utf8_continuations_and_adjusts_start_offset() {
        let inner = std::io::Cursor::new(Vec::<u8>::new());
        let mut rr = RingReader::new(inner);

        // Leading continuation bytes (0x82, 0xAC) without the preceding lead byte.
        // Then ASCII bytes.
        rr.push_ring_bytes(&[0x82, 0xAC, b'a', b'b'], 100);

        let snap = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap);

        assert_eq!(snap.start_offset, 102);
        assert_eq!(snap.bytes, vec![b'a', b'b']);
        assert_eq!(snap.end_offset, 104);
    }

    #[test]
    fn get_recent_trims_trailing_incomplete_utf8_sequence() {
        let inner = std::io::Cursor::new(Vec::<u8>::new());
        let mut rr = RingReader::new(inner);

        // 'abc' + 0xE2 (lead byte of 3-byte UTF-8 sequence) => incomplete at end.
        rr.push_ring_bytes(&[b'a', b'b', b'c', 0xE2], 5);

        let snap = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap);

        assert_eq!(snap.start_offset, 5);
        assert_eq!(snap.bytes, b"abc".to_vec());
        assert_eq!(snap.end_offset, 8);
    }

    #[test]
    fn snapshot_contains_read_ahead_bytes_and_grows_when_refilled() {
        let data = make_ascii_data(MAX_READ_AHEAD + 10);
        let inner = CountingReader::with_max_chunk(data.clone(), 13);
        let mut rr = RingReader::new(inner);

        // First snapshot reads ahead to cap (or data length if smaller).
        let snap1 = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap1);
        assert_eq!(snap1.bytes.len(), MAX_READ_AHEAD);
        assert_eq!(&snap1.bytes[..], &data[..MAX_READ_AHEAD]);

        // Consume 1 byte from stash.
        let mut one = [0u8; 1];
        rr.read_exact(&mut one).unwrap();
        assert_eq!(one[0], data[0]);
        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD - 1);

        // Next snapshot should read exactly 1 byte to restore read-ahead to cap,
        // and ring should now include that new byte too.
        let snap2 = rr.get_recent().unwrap();
        assert_snapshot_offsets_consistent(&snap2);

        assert_eq!(rr.read_ahead_len(), MAX_READ_AHEAD);
        assert_eq!(snap2.start_offset, 0);
        assert_eq!(snap2.bytes.len(), MAX_READ_AHEAD + 1);
        assert_eq!(&snap2.bytes[..], &data[..MAX_READ_AHEAD + 1]);
    }
}
