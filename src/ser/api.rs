use super::options::SerializerOptions;

// ---------------- Serialization (public API) ----------------

/// Serialize a value to a YAML `String`.
///
/// This is the easiest entry point when you just want a YAML string.
///
/// Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Foo { a: i32, b: bool }
///
/// let s = serde_saphyr::to_string(&Foo { a: 1, b: true }).unwrap();
/// assert!(s.contains("a: 1"));
/// ```
#[cfg(feature = "serialize")]
pub fn to_string<T: serde::Serialize>(value: &T) -> std::result::Result<String, crate::ser::Error> {
    let mut out = String::new();
    to_fmt_writer(&mut out, value)?;
    Ok(out)
}

/// Serialize a value to a YAML `String`, with [`SerializerOptions`].
///
/// This is like [`to_string`], but lets you control formatting and serialization
/// behavior through the provided `options`.
///
/// Example
///
/// ```rust
/// use serde::Serialize;
/// use serde_saphyr::SerializerOptions;
///
/// #[derive(Serialize)]
/// struct Foo { a: i32, b: bool }
///
/// let options = SerializerOptions::default();
/// let s = serde_saphyr::to_string_with_options(&Foo { a: 1, b: true }, options).unwrap();
/// assert!(s.contains("a: 1"));
/// ```
#[cfg(feature = "serialize")]
pub fn to_string_with_options<T: serde::Serialize>(
    value: &T,
    options: SerializerOptions,
) -> std::result::Result<String, crate::ser::Error> {
    let mut out = String::new();
    to_fmt_writer_with_options(&mut out, value, options)?;
    Ok(out)
}

/// Deprecated: use `to_fmt_writer` or `to_io_writer`
/// Kept for a transition release to avoid instant breakage.
#[deprecated(
    since = "0.0.7",
    note = "Use `to_fmt_writer` for `fmt::Write` (String, fmt::Formatter) or `to_io_writer` for files/sockets."
)]
#[cfg(feature = "serialize")]
pub fn to_writer<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
) -> std::result::Result<(), crate::ser::Error> {
    let mut ser = crate::ser::YamlSerializer::new(output);
    value.serialize(&mut ser)
}

/// Serialize a value as YAML into any [`std::fmt::Write`] target.
#[cfg(feature = "serialize")]
pub fn to_fmt_writer<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
) -> std::result::Result<(), crate::ser::Error> {
    to_fmt_writer_with_options(output, value, SerializerOptions::default())
}

/// Serialize a value as YAML into any [`std::io::Write`] target.
#[cfg(feature = "serialize")]
pub fn to_io_writer<W: std::io::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
) -> std::result::Result<(), crate::ser::Error> {
    to_io_writer_with_options(output, value, SerializerOptions::default())
}

/// Serialize a value as YAML into any [`std::fmt::Write`] target, with options.
/// Options are consumed because anchor generator may be taken from them.
#[cfg(feature = "serialize")]
pub fn to_fmt_writer_with_options<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
    mut options: SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    options.consistent()?;
    let mut ser = crate::ser::YamlSerializer::with_options(output, &mut options);
    value.serialize(&mut ser)
}

/// Serialize a value as YAML into any [`std::io::Write`] target, with options.
/// Options are consumed because anchor generator may be taken from them.
#[cfg(feature = "serialize")]
pub fn to_io_writer_with_options<W: std::io::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
    mut options: SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    options.consistent()?;
    struct Adapter<'a, W: std::io::Write> {
        output: &'a mut W,
        last_err: Option<std::io::Error>,
    }
    impl<'a, W: std::io::Write> std::fmt::Write for Adapter<'a, W> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if let Err(e) = self.output.write_all(s.as_bytes()) {
                self.last_err = Some(e);
                return Err(std::fmt::Error);
            }
            Ok(())
        }
        fn write_char(&mut self, c: char) -> std::fmt::Result {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            self.write_str(s)
        }
    }
    let mut adapter = Adapter {
        output,
        last_err: None,
    };
    let mut ser = crate::ser::YamlSerializer::with_options(&mut adapter, &mut options);
    match value.serialize(&mut ser) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(io_error) = adapter.last_err.take() {
                return Err(crate::ser::Error::from(io_error));
            }
            Err(e)
        }
    }
}

/// Deprecated: use `to_fmt_writer_with_options` for `fmt::Write` or `to_io_writer_with_options` for `io::Write`.
#[deprecated(
    since = "0.0.7",
    note = "Use `to_fmt_writer_with_options` for fmt::Write or `to_io_writer_with_options` for io::Write."
)]
#[cfg(feature = "serialize")]
pub fn to_writer_with_options<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
    options: SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    to_fmt_writer_with_options(output, value, options)
}

/// Serialize multiple documents into a YAML string.
///
/// Serializes each value in the provided slice as an individual YAML document.
/// Documents are separated by a standard YAML document start marker ("---\n").
/// No marker is emitted before the first document.
///
/// Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32 }
///
/// let docs = vec![Point { x: 1 }, Point { x: 2 }];
/// let out = serde_saphyr::to_string_multiple(&docs).unwrap();
/// assert_eq!(out, "x: 1\n---\nx: 2\n");
/// ```
#[cfg(feature = "serialize")]
pub fn to_string_multiple<T: serde::Serialize>(
    values: &[T],
) -> std::result::Result<String, crate::ser::Error> {
    to_string_multiple_with_options(values, SerializerOptions::default())
}

/// Serialize multiple documents into a YAML string with configurable `Options`.
///
/// Serializes each value in the provided slice as an individual YAML document.
/// Documents are separated by a standard YAML document start marker ("---\n").
/// No marker is emitted before the first document.
///
/// Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { coords: Vec<i32> }
///
/// let docs = vec![Point { coords: vec![0,1] }, Point { coords: vec![3,2] }];
/// let options = serde_saphyr::ser_options! {
///     indent_step: 2,
///     compact_list_indent: true
/// };
/// let out = serde_saphyr::to_string_multiple_with_options(&docs, options).unwrap();
/// assert_eq!(out, "coords:\n- 0\n- 1\n---\ncoords:\n- 3\n- 2\n");
/// ```
#[cfg(feature = "serialize")]
pub fn to_string_multiple_with_options<T: serde::Serialize>(
    values: &[T],
    options: SerializerOptions,
) -> std::result::Result<String, crate::ser::Error> {
    let mut out = String::new();
    let mut first = true;
    for v in values {
        if !first {
            out.push_str("---\n");
        }
        first = false;
        to_fmt_writer_with_options(&mut out, v, options)?;
    }
    Ok(out)
}
