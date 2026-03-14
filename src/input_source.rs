use std::io::Read;

/// Owned input that can be fed into the YAML parser.
///
/// This is primarily used by the include resolver: it can return either fully-owned
/// in-memory text, or a fully-owned streaming reader.
pub enum InputSource {
    /// Owned text.
    Text(String),
    /// Owned YAML text together with the name of an anchor to extract from it.
    ///
    /// This is mainly intended for resolvers that support include specs such as
    /// `path/to/file.yaml#anchor_name`. In that case, the resolver still receives the full
    /// include spec via [`IncludeRequest::spec`], splits the file part from the fragment itself,
    /// reads the target document, and returns:
    ///
    /// ```rust
    /// # use serde_saphyr::InputSource;
    /// let source = InputSource::AnchoredText {
    ///     text: "defaults: &defaults\n  enabled: true\nfeature: *defaults\n".to_owned(),
    ///     anchor: "defaults".to_owned(),
    /// };
    /// ```
    ///
    /// During parsing, `serde-saphyr` will parse `text`, find the node tagged with `&defaults`,
    /// and replay only that anchored node as the included value. Conceptually, this makes:
    ///
    /// ```yaml
    /// settings: !include config.yaml#defaults
    /// ```
    ///
    /// behave as if `settings` directly contained the YAML node anchored as `&defaults` inside
    /// `config.yaml`.
    ///
    /// Use [`InputSource::Text`] when the whole document should be included, and use
    /// [`InputSource::AnchoredText`] only when you want the include to resolve to a specific
    /// anchored fragment.
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

/// Specific problems encountered during file include resolution.
#[derive(Debug)]
#[non_exhaustive]
pub enum ResolveProblem {
    /// Failed to canonicalize the include target path.
    ResolveFailed { spec: String, base_dir: String, err: std::io::Error },
    /// The include target is not a regular file.
    TargetNotRegularFile { target: String },
    /// The include target resolves to the configured root file itself (cyclic include).
    TargetIsRootFile { spec: String },
    /// The parent include id was not an absolute canonical path.
    ParentIdNotAbsoluteCanonical { parent_id: String },
    /// Failed to resolve the parent include source.
    ParentResolveFailed { parent_id: String, from_name: String, err: std::io::Error },
    /// The parent include is not a regular file.
    ParentNotRegularFile { parent: String },
    /// The parent include does not have a parent directory.
    ParentHasNoDirectory { parent: String },
    /// The include resolves outside the configured root directory.
    ResolvesOutsideRoot { spec: String, root: String },
    /// The include traverses a symlink, which is disabled by policy.
    TraversesSymlink { spec: String },
    /// Absolute include paths are not allowed.
    AbsolutePathNotAllowed { spec: String },
    /// The include path is empty.
    EmptyPath,
    /// The include target does not have a valid YAML extension (.yml or .yaml).
    InvalidExtension { spec: String },
    /// The include target is a hidden file (starts with a dot).
    HiddenFile { spec: String },
    /// The include fragment is empty.
    EmptyFragment,
    /// The include fragment contains a '#' character.
    FragmentContainsHash { spec: String },
}

/// Error type returned by user-provided include resolvers.
#[derive(Debug)]
#[non_exhaustive]
pub enum IncludeResolveError {
    Io(std::io::Error),
    Message(String),
    SizeLimitExceeded(usize, usize),
    FileInclude(Box<ResolveProblem>),
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
    /// Remaining decoded byte quota available for additional reader-backed input, if configured.
    pub size_remaining: Option<usize>,
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
