#![no_main]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read};

use libfuzzer_sys::fuzz_target;
use serde::de::IgnoredAny;
use serde_saphyr::{DuplicateKeyPolicy, Options};

fn options(selector: u8) -> Options {
    serde_saphyr::options! {
        duplicate_keys: match selector % 3 {
            0 => DuplicateKeyPolicy::Error,
            1 => DuplicateKeyPolicy::FirstWins,
            _ => DuplicateKeyPolicy::LastWins,
        },
        reject_unsupported_tags: selector & 8 != 0,
        no_schema: selector & 16 != 0,
        legacy_octal_numbers: selector & 32 != 0,
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(64 * 1024),
            max_events: 16_384,
            max_nodes: 8_192,
            max_documents: 64,
            max_anchors: 128,
            max_aliases: 256,
            max_recorded_anchor_events: 16_384,
            max_recorded_anchor_bytes: 1024 * 1024,
            max_total_scalar_bytes: 1024 * 1024,
            max_total_comment_bytes: 64 * 1024,
        },
        alias_limits: serde_saphyr::alias_limits! {
            max_total_replayed_events: 16_384,
            max_alias_expansions_per_anchor: 256,
        },
    }
}

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
    ($data:expr, $options:expr, $target:ty) => {
        let _ = serde_saphyr::from_slice_with_options::<$target>($data, $options.clone());
    };
}

fuzz_target!(|data: &[u8]| {
    // Bound per-input work independently of serde-saphyr's own budgets.
    if data.len() > 64 * 1024 {
        return;
    }

    let options = options(data.first().copied().unwrap_or_default());
    exercise_slice_type!(data, options, IgnoredAny);
    exercise_slice_type!(data, options, serde_json::Value);
    exercise_slice_type!(data, options, bool);
    exercise_slice_type!(data, options, i64);
    exercise_slice_type!(data, options, u64);
    exercise_slice_type!(data, options, f64);
    exercise_slice_type!(data, options, String);
    exercise_slice_type!(data, options, Option<String>);
    exercise_slice_type!(data, options, Vec<IgnoredAny>);
    exercise_slice_type!(data, options, BTreeMap<String, IgnoredAny>);
    exercise_slice_type!(data, options, BTreeMap<Vec<Option<String>>, IgnoredAny>);

    let _ = serde_saphyr::from_slice_multiple_with_options::<IgnoredAny>(data, options.clone());

    let _ =
        serde_saphyr::from_reader_with_options::<_, IgnoredAny>(Cursor::new(data), options.clone());

    let chunk_size = data.first().map_or(1, |byte| usize::from(*byte % 32) + 1);

    let mut chunked = Chunked {
        input: data,
        position: 0,
        chunk_size,
    };
    // Exercise recovery after deserialization errors as well as small reader chunks.
    for result in
        serde_saphyr::read_with_options::<_, serde_json::Value>(&mut chunked, options.clone())
            .take(64)
    {
        let _ = result;
    }

    let fail_at = data
        .get(1)
        .map_or(data.len(), |byte| usize::from(*byte) % (data.len() + 1));

    let _ = serde_saphyr::from_reader_with_options::<_, IgnoredAny>(
        Faulting {
            input: data,
            position: 0,
            fail_at,
            emitted_error: false,
        },
        options.clone(),
    );

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = serde_saphyr::from_str_with_options::<IgnoredAny>(text, options.clone());
        let _ = serde_saphyr::from_multiple_with_options::<IgnoredAny>(text, options);
    }
});
