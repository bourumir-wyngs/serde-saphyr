use std::io::Read;

/// Owned input that can be fed into the YAML parser.
///
/// This is primarily used by the include resolver: it can return either fully-owned
/// in-memory text, or a fully-owned streaming reader.
pub enum InputSource {
    /// Owned text.
    Text(String),
    /// Owned reader (streaming).
    Reader(Box<dyn Read + 'static>),
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

/// A request passed to the include resolver to resolve an include directive.
pub struct IncludeRequest<'a> {
    /// The include specification (e.g. the path or URL).
    pub spec: &'a str,
    /// The name of the file or source currently being parsed (top of the include stack).
    pub from_name: &'a str,
    /// The canonical identity of the source currently being parsed, or None for the root parser.
    pub from_id: Option<&'a str>,
    /// The full chain of inclusions leading to this request, with the current file at the end.
    pub stack: Vec<String>,
    /// The location in the source file where the include was requested.
    pub location: crate::Location,
}

/// A type alias for the include resolver closure.
pub type IncludeResolver<'a> = dyn FnMut(IncludeRequest<'_>) -> Result<ResolvedInclude, IncludeResolveError> + 'a;

impl InputSource {
    #[inline]
    pub fn from_string(s: String) -> Self {
        Self::Text(s)
    }

    #[inline]
    pub fn from_reader<R>(r: R) -> Self
    where
        R: Read + 'static,
    {
        Self::Reader(Box::new(r))
    }
}
