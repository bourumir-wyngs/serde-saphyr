#![no_main]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read};

use libfuzzer_sys::fuzz_target;
use serde::de::IgnoredAny;

struct Chunked<'a> {
    input: &'a [u8],
    position: usize,
    chunk_size: usize,
}

impl Read for Chunked<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.position == self.input.len() {
            return Ok(0);
        }

        let count = output
            .len()
            .min(self.chunk_size)
            .min(self.input.len() - self.position);

        output[..count].copy_from_slice(&self.input[self.position..self.position + count]);

        self.position += count;

        Ok(count)
    }
}

struct Faulting<'a> {
    input: &'a [u8],
    position: usize,
    fail_at: usize,
    emitted_error: bool,
}

impl Read for Faulting<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        if !self.emitted_error && self.position >= self.fail_at {
            self.emitted_error = true;

            return Err(io::Error::other("fuzz-injected reader failure"));
        }

        if self.position == self.input.len() {
            return Ok(0);
        }

        let bytes_before_failure = if self.emitted_error {
            self.input.len() - self.position
        } else {
            self.fail_at.saturating_sub(self.position)
        };

        let count = output
            .len()
            .min(bytes_before_failure)
            .min(self.input.len() - self.position);

        if count == 0 {
            return Ok(0);
        }

        output[..count].copy_from_slice(&self.input[self.position..self.position + count]);

        self.position += count;

        Ok(count)
    }
}

macro_rules! exercise_slice_type {
    ($data:expr, $target:ty) => {
        let _ = serde_saphyr::from_slice::<$target>($data);
    };
}

fuzz_target!(|data: &[u8]| {
    // Bound per-input work independently of serde-saphyr's own budgets.
    if data.len() > 64 * 1024 {
        return;
    }

    exercise_slice_type!(data, IgnoredAny);
    exercise_slice_type!(data, bool);
    exercise_slice_type!(data, i64);
    exercise_slice_type!(data, u64);
    exercise_slice_type!(data, f64);
    exercise_slice_type!(data, String);
    exercise_slice_type!(data, Option<String>);
    exercise_slice_type!(data, Vec<IgnoredAny>);
    exercise_slice_type!(data, BTreeMap<String, IgnoredAny>);

    let _ = serde_saphyr::from_slice_multiple::<IgnoredAny>(data);

    let _ = serde_saphyr::from_reader::<_, IgnoredAny>(Cursor::new(data));

    let chunk_size = data.first().map_or(1, |byte| usize::from(*byte % 32) + 1);

    let _ = serde_saphyr::from_reader::<_, IgnoredAny>(Chunked {
        input: data,
        position: 0,
        chunk_size,
    });

    let fail_at = data
        .get(1)
        .map_or(data.len(), |byte| usize::from(*byte) % (data.len() + 1));

    let _ = serde_saphyr::from_reader::<_, IgnoredAny>(Faulting {
        input: data,
        position: 0,
        fail_at,
        emitted_error: false,
    });

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = serde_saphyr::from_str::<IgnoredAny>(text);
        let _ = serde_saphyr::from_multiple::<IgnoredAny>(text);
    }
});
