use std::io::Read;

/// Owned input that can be fed into the YAML parser.
///
/// This is primarily used by the include resolver: it can return either fully-owned
/// in-memory text, or a fully-owned streaming reader.
pub enum InputSource {
    /// Owned text.
    Text(String),
    /// Owned reader (streaming).
    Reader(Box<dyn Read + Send + 'static>),
}

impl InputSource {
    #[inline]
    pub fn from_string(s: String) -> Self {
        Self::Text(s)
    }

    #[inline]
    pub fn from_reader<R>(r: R) -> Self
    where
        R: Read + Send + 'static,
    {
        Self::Reader(Box::new(r))
    }
}
