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

/// A resolved include containing the source identity and the content.
pub struct ResolvedInclude {
    /// The canonical identity of the included source, used for cycle detection and absolute paths.
    pub id: String,
    /// The display name of the included source, used for error messages.
    pub name: String,
    /// The actual content to parse.
    pub source: InputSource,
}

/// Error type returned by user-provided include resolvers.
#[derive(Debug)]
pub enum IncludeResolveError {
    Io(std::io::Error),
    Message(String),
}

impl From<std::io::Error> for IncludeResolveError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// A type alias for the include resolver closure.
pub type IncludeResolver<'a> = dyn FnMut(&str) -> Result<ResolvedInclude, IncludeResolveError> + 'a;

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
