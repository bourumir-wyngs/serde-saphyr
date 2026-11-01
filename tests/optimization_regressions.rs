#![cfg(feature = "serialize")]

use serde_saphyr::{SingleQuoted, to_string};

#[test]
fn single_quoted_literal_runs_preserve_utf8_at_apostrophe_boundaries() {
    for (value, expected) in [
        ("", "''\n"),
        ("é中🙂", "'é中🙂'\n"),
        ("'é中🙂''終🙂'", "'''é中🙂''''終🙂'''\n"),
    ] {
        let yaml = to_string(&SingleQuoted(value)).unwrap();
        assert_eq!(yaml, expected);
        #[cfg(feature = "deserialize")]
        assert_eq!(serde_saphyr::from_str::<String>(&yaml).unwrap(), value);
    }
}

#[test]
fn single_quoted_writer_failure_after_literal_prefix_is_preserved() {
    struct LimitedWriter {
        bytes: Vec<u8>,
        limit: usize,
        failures: usize,
    }

    impl std::io::Write for LimitedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len() == self.limit {
                self.failures += 1;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "failure after literal prefix",
                ));
            }
            let count = bytes.len().min(self.limit - self.bytes.len());
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let value = SingleQuoted("pré🙂'終");
    let expected = "'pré🙂''終'\n";
    // Fail during the ordinary run, at its end, and inside the doubled apostrophe.
    for limit in [3, "'pré🙂".len(), "'pré🙂'".len()] {
        let mut writer = LimitedWriter {
            bytes: Vec::new(),
            limit,
            failures: 0,
        };
        let error = serde_saphyr::to_io_writer(&mut writer, &value).unwrap_err();
        match error {
            serde_saphyr::ser_error::Error::IO { error } => {
                assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
                assert_eq!(error.to_string(), "failure after literal prefix");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(writer.bytes, &expected.as_bytes()[..limit]);
        assert_eq!(writer.failures, 1);
    }
}

#[test]
fn automatic_folding_uses_character_threshold_not_utf8_bytes() {
    for (value, folded) in [
        ("abcdefg", false),
        ("abcdefgh", false),
        ("abcdefghi", true),
        ("ééééé", false),
        ("éééééééé", false),
        ("ééééééééé", true),
        ("界界界界", false),
        ("🙂🙂🙂🙂🙂🙂🙂🙂", false),
        ("🙂🙂🙂🙂🙂🙂🙂🙂🙂", true),
    ] {
        let yaml = serde_saphyr::to_string_with_options(
            &value,
            serde_saphyr::ser_options! { folded_wrap_chars: 8 },
        )
        .unwrap();
        assert_eq!(yaml.starts_with(">-\n"), folded, "{value:?}: {yaml:?}");
        if !folded {
            assert_eq!(yaml, format!("{value}\n"));
        }
        #[cfg(feature = "deserialize")]
        assert_eq!(serde_saphyr::from_str::<String>(&yaml).unwrap(), value);
    }
}

#[test]
fn comment_detection_preserves_unicode_whitespace_before_hash() {
    for whitespace in ['\u{a0}', '\u{2003}', '\u{202f}', '\u{3000}'] {
        let value = format!("é{whitespace}#終");
        let yaml = to_string(&value).unwrap();
        assert_eq!(yaml, format!("\"{value}\"\n"));

        let map = std::collections::BTreeMap::from([(&value, &value)]);
        assert_eq!(
            to_string(&map).unwrap(),
            format!("\"{value}\": \"{value}\"\n")
        );
        assert_eq!(
            to_string(&serde_saphyr::FlowSeq([&value])).unwrap(),
            format!("[\"{value}\"]\n")
        );
        #[cfg(feature = "deserialize")]
        assert_eq!(serde_saphyr::from_str::<String>(&yaml).unwrap(), value);
    }
    assert_eq!(to_string(&"é#終").unwrap(), "é#終\n");
}
