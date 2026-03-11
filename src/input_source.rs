use std::io::Read;

/// Owned input that can be fed into the YAML parser.
///
/// This is primarily used by the include resolver: it can return either fully-owned
/// in-memory text, or a fully-owned streaming reader.
pub enum InputSource {
    /// Owned text.
    Text(String),
    /// Owned text together with an anchor fragment that should become the first parsed node.
    AnchoredText { text: String, anchor: String },
    /// Owned reader (streaming).
    Reader(Box<dyn Read + 'static>),
}

impl std::fmt::Debug for InputSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => f.debug_tuple("Text").field(text).finish(),
            Self::AnchoredText { text, anchor } => f
                .debug_struct("AnchoredText")
                .field("anchor", anchor)
                .field("text", text)
                .finish(),
            Self::Reader(_) => f.write_str("Reader(..)"),
        }
    }
}

/// A resolved include containing the source identity and the content.
#[derive(Debug)]
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

/// Callback used to resolve `!include` directives during parsing.
///
/// The resolver receives an [`IncludeRequest`] describing what was requested, from which
/// source it originated, and where in the source file the directive was encountered. It must
/// either return a [`ResolvedInclude`] with a stable `id`, human-friendly `name`, and the
/// replacement [`InputSource`], or fail with [`IncludeResolveError`].
///
/// The `id` should uniquely identify the underlying resource after any normalization you need
/// (for example, a canonical filesystem path or a normalized URL). `serde-saphyr` uses this
/// identifier for include-stack tracking and cycle detection. The `name` is intended for error
/// messages and can be more user-friendly.
///
/// Resolvers may return:
/// - [`InputSource::Text`] for ordinary in-memory YAML,
/// - [`InputSource::AnchoredText`] when the include should behave as if a specific anchor was
///   the first parsed node, or
/// - [`InputSource::Reader`] when content should be streamed from an owned reader.
///
/// A resolver is invoked lazily, when a `!include` tag is encountered. Because the type is
/// `FnMut`, the callback may keep state such as caches, metrics, or a virtual file map.
///
/// ```rust
/// # #[cfg(feature = "include")]
/// # {
/// use serde::Deserialize;
/// use serde_saphyr::{
///     from_str_with_options, options, IncludeRequest, IncludeResolveError, InputSource,
///     ResolvedInclude,
/// };
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     users: Vec<User>,
/// }
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct User {
///     name: String,
/// }
///
/// let root_yaml = "users: !include virtual://users.yaml\n";
/// let users_yaml = "- name: Alice\n- name: Bob\n";
///
/// let options = options! {}.with_include_resolver(|req: IncludeRequest<'_>| {
///     assert_eq!(req.spec, "virtual://users.yaml");
///     assert_eq!(req.from_id, None);
///     assert_eq!(req.from_name, "<input>");
///
///     if req.spec == "virtual://users.yaml" {
///         Ok(ResolvedInclude {
///             id: req.spec.to_owned(),
///             name: "virtual users".to_owned(),
///             source: InputSource::from_string(users_yaml.to_owned()),
///         })
///     } else {
///         Err(IncludeResolveError::Message(format!("unknown include: {}", req.spec)))
///     }
/// });
///
/// let config: Config = from_str_with_options(root_yaml, options).unwrap();
/// assert_eq!(config.users.len(), 2);
/// assert_eq!(config.users[0].name, "Alice");
/// # }
/// ```
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
