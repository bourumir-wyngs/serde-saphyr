#![no_main]

use std::io::{self, Read};

use libfuzzer_sys::fuzz_target;

const MAX_EXTRA_DOCUMENTS: usize = 16;

struct Chunked<'a> {
    remaining: &'a [u8],
    chunk_size: usize,
}

impl Read for Chunked<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = output.len().min(self.chunk_size).min(self.remaining.len());
        output[..count].copy_from_slice(&self.remaining[..count]);
        self.remaining = &self.remaining[count..];
        Ok(count)
    }
}

fn byte(data: &[u8], index: usize) -> u8 {
    data.get(index).copied().unwrap_or(0)
}

fn document_stream(data: &[u8]) -> (String, Vec<Option<String>>) {
    // Five control bytes followed by at most 16 scalar-document selectors.
    // Missing bytes default to zero, so an empty input tests an empty literal
    // block followed by another document: the original boundary regression.
    let flags = byte(data, 0);
    let block = byte(data, 1);
    let folded = block & 1 != 0;
    let chomping = (block >> 1) % 3; // clip, strip, keep
    let indentation = (block >> 3) % 3; // root, inferred two spaces, explicit two
    let blank_lines = usize::from(byte(data, 3) % 4);
    let separator = [
        "--- ",
        "---\t",
        "...\n--- ",
        "...\n# between documents\n---\n",
        "--- # next document\n",
    ][usize::from(byte(data, 4)) % 5];

    // Fixed safe content permits a semantic oracle independent of the parser.
    // Prefixes resembling document markers are content unless followed by
    // whitespace; Unicode also exercises UTF-8 split across reader chunks.
    const CONTENTS: &[&[&str]] = &[
        &[],
        &["null"],
        &["~"],
        &["Null"],
        &["NULL"],
        &["true"],
        &["---text"],
        &["...text"],
        &["first", "second"],
        &["μ", "雪"],
    ];
    let lines = CONTENTS[usize::from(byte(data, 2)) % CONTENTS.len()];

    let mut yaml = String::new();
    if flags & 2 != 0 {
        yaml.push_str("--- ");
    }
    yaml.push(if folded { '>' } else { '|' });
    if indentation == 2 {
        yaml.push('2');
    }
    match chomping {
        1 => yaml.push('-'),
        2 => yaml.push('+'),
        _ => {}
    }
    if block & 64 != 0 {
        yaml.push_str(" # block header");
    }
    yaml.push('\n');
    for line in lines {
        if indentation != 0 {
            yaml.push_str("  ");
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    yaml.push_str(&"\n".repeat(blank_lines));

    let mut block_value = lines.join(if folded { " " } else { "\n" });
    if !lines.is_empty() && chomping != 1 {
        block_value.push('\n');
    }
    if chomping == 2 {
        block_value.push_str(&"\n".repeat(blank_lines));
    }
    let mut expected = vec![Some(block_value)];

    // The stream APIs deliberately skip actual null documents, but every
    // string representation of a null-like value must remain in the output.
    const SCALARS: &[(&str, Option<&str>)] = &[
        ("null", None),
        ("~", None),
        ("!!null", None),
        ("!!null null", None),
        ("!!null ~", None),
        ("", None),
        ("!!str null", Some("null")),
        ("!!str ~", Some("~")),
        ("!!str", Some("")),
        ("! null", Some("null")),
        ("! ~", Some("~")),
        ("!", Some("")),
        ("'null'", Some("null")),
        ("'~'", Some("~")),
        ("''", Some("")),
        ("\"μ雪\"", Some("μ雪")),
    ];
    for selector in data
        .get(5..)
        .unwrap_or_default()
        .iter()
        .take(MAX_EXTRA_DOCUMENTS)
    {
        let (scalar, value) = SCALARS[usize::from(*selector) % SCALARS.len()];
        yaml.push_str(separator);
        yaml.push_str(scalar);
        yaml.push('\n');
        if let Some(value) = value {
            expected.push(Some(value.to_owned()));
        }
    }

    // Keep a non-null sentinel after the block even when the input is empty.
    // It detects a consumed document marker and guarantees multiple documents
    // for the single-document API assertions below.
    yaml.push_str(separator);
    yaml.push_str("'kept'\n");
    expected.push(Some("kept".to_owned()));
    if flags & 4 != 0 {
        yaml.push_str("...\n");
    }
    if flags & 1 != 0 {
        yaml = yaml.replace('\n', "\r\n");
    }
    (yaml, expected)
}

fuzz_target!(|data: &[u8]| {
    // Only the first 21 bytes are used. Each input produces at most 18 shallow
    // documents and less than 2 KiB of YAML, independently of fuzz input size.
    let (yaml, expected) = document_stream(data);
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(4096),
            max_documents: 32,
            max_events: 256,
            max_nodes: 64,
            max_depth: 16,
            max_total_scalar_bytes: 4096,
            max_total_comment_bytes: 4096,
        },
    };

    let multiple: Vec<Option<String>> =
        serde_saphyr::from_multiple_with_options(&yaml, options.clone())
            .unwrap_or_else(|error| panic!("from_multiple: {error:?}\n{yaml}"));
    assert_eq!(multiple, expected, "from_multiple: {yaml}");

    let slices: Vec<Option<String>> =
        serde_saphyr::from_slice_multiple_with_options(yaml.as_bytes(), options.clone())
            .unwrap_or_else(|error| panic!("from_slice_multiple: {error:?}\n{yaml}"));
    assert_eq!(slices, expected, "from_slice_multiple: {yaml}");

    let chunk_size = usize::from(byte(data, 0) >> 3) + 1;
    let mut reader = Chunked {
        remaining: yaml.as_bytes(),
        chunk_size,
    };
    let streamed: Vec<Option<String>> =
        serde_saphyr::read_with_options(&mut reader, options.clone())
            .take(MAX_EXTRA_DOCUMENTS + 3)
            .collect::<Result<_, _>>()
            .unwrap_or_else(|error| panic!("read: {error:?}\n{yaml}"));
    assert_eq!(
        streamed, expected,
        "read in {chunk_size}-byte chunks: {yaml}"
    );

    let single_documents = [
        serde_saphyr::from_str_with_options::<Option<String>>(&yaml, options.clone()),
        serde_saphyr::from_slice_with_options::<Option<String>>(yaml.as_bytes(), options.clone()),
        serde_saphyr::from_reader_with_options::<_, Option<String>>(
            Chunked {
                remaining: yaml.as_bytes(),
                chunk_size,
            },
            options,
        ),
    ];
    for result in single_documents {
        let error = result.expect_err("single-document API accepted multiple documents");
        assert!(
            matches!(
                error.without_snippet(),
                serde_saphyr::Error::MultipleDocuments { .. }
            ),
            "wrong single-document error: {error:?}\n{yaml}",
        );
    }
});
