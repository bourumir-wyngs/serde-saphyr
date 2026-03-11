#[cfg(feature = "include")]
use crate::input_source::{IncludeRequest, IncludeResolveError, InputSource, ResolvedInclude};
#[cfg(feature = "include")]
use encoding_rs_io::DecodeReaderBytesBuilder;
#[cfg(feature = "include")]
use std::fs;
#[cfg(feature = "include")]
use std::io::{self, Read};
#[cfg(feature = "include")]
use std::path::{Component, Path, PathBuf};

/// How [`SafeFileResolver`] should hand included files to the parser.
#[cfg(feature = "include")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SafeFileReadMode {
    /// Read and decode the file eagerly into a `String`.
    ///
    /// This is the default because it gives the best error snippets for nested includes while
    /// still using BOM-aware decoding for common Unicode encodings.
    #[default]
    Text,
    /// Stream the file through a reader.
    ///
    /// This may use less memory for large inputs, but nested include diagnostics are weaker
    /// because the full included source text is not retained.
    Reader,
}

/// Policy for symlink handling in [`SafeFileResolver`].
#[cfg(feature = "include")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymlinkPolicy {
    /// Follow symlinks, but only if the final canonical target remains inside the configured root.
    #[default]
    FollowWithinRoot,
    /// Reject any include path that traverses a symlink.
    Reject,
}

/// A filesystem-backed include resolver that confines all resolved files to a configured root.
///
/// This resolver is meant for `!include` use cases where you want a safe default for local files:
///
/// - include paths must be relative
/// - targets are canonicalized before use
/// - resolved targets must stay inside the configured root directory
/// - display names are made relative to that root when possible
///
/// Typical usage is to configure a resolver once and hand it to the deserializer for every
/// `!include` lookup:
///
/// ```no_run
/// use serde::Deserialize;
/// use serde_saphyr::{from_str_with_options, Options, SafeFileResolver};
///
/// #[derive(Debug, Deserialize)]
/// struct Config {
///     name: String,
/// }
///
/// let yaml = "name: demo\nchild: !include child.yaml\n";
/// let resolver = SafeFileResolver::new("./configs")?;
/// let options = Options::default().with_include_resolver(resolver.into_callback());
/// let config: Config = from_str_with_options(yaml, options)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// By default, files are decoded into text with BOM-aware Unicode decoding. That preserves better
/// snippet diagnostics for nested includes. If you prefer streaming, switch to
/// [`SafeFileReadMode::Reader`].
#[cfg(feature = "include")]
#[derive(Clone, Debug)]
pub struct SafeFileResolver {
    /// Canonical directory that forms the hard security boundary for all resolved include targets.
    allow_root: PathBuf,
    /// Canonical directory used to resolve top-level includes when there is no parent include file.
    root_base_dir: PathBuf,
    /// Canonical identifier of the configured root file, used to detect self-inclusion early.
    root_source_id: Option<String>,
    /// Controls whether included files are decoded eagerly into text or streamed to the parser.
    read_mode: SafeFileReadMode,
    /// Determines whether symlinks are followed within the root or rejected outright.
    symlink_policy: SymlinkPolicy,
}

#[cfg(feature = "include")]
impl SafeFileResolver {
    /// Create a resolver confined to `allow_root`.
    ///
    /// Top-level includes (those requested from the root parser input, where `from_id` is absent)
    /// are resolved relative to this same directory unless you later call
    /// [`SafeFileResolver::with_root_base_dir`] or [`SafeFileResolver::with_root_file`].
    pub fn new<P>(allow_root: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let allow_root = canonicalize_existing_dir(allow_root.as_ref())?;
        Ok(Self {
            root_base_dir: allow_root.clone(),
            allow_root,
            root_source_id: None,
            read_mode: SafeFileReadMode::Text,
            symlink_policy: SymlinkPolicy::FollowWithinRoot,
        })
    }

    /// Set the base directory used for top-level includes.
    ///
    /// The directory must already exist and must remain inside the configured root.
    /// Calling this clears any previously configured root-file identity.
    pub fn with_root_base_dir<P>(mut self, root_base_dir: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let root_base_dir = canonicalize_existing_dir(root_base_dir.as_ref())?;
        ensure_inside_root_io(&self.allow_root, &root_base_dir, "root base directory")?;
        self.root_base_dir = root_base_dir;
        self.root_source_id = None;
        Ok(self)
    }

    /// Set the root file that the caller is parsing.
    ///
    /// This adjusts top-level include resolution to use the parent directory of `root_file`, while
    /// still confining all resolved targets to the configured root. It also remembers the canonical
    /// identity of the root file so `!include root.yaml` can be rejected immediately instead of
    /// recursing once before cycle detection catches it.
    pub fn with_root_file<P>(mut self, root_file: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let root_file = canonicalize_existing_file(root_file.as_ref())?;
        ensure_inside_root_io(&self.allow_root, &root_file, "root file")?;
        let Some(parent) = root_file.parent() else {
            return Err(invalid_input("root file does not have a parent directory"));
        };
        self.root_base_dir = parent.to_path_buf();
        self.root_source_id = Some(path_to_string(&root_file));
        Ok(self)
    }

    /// Choose whether files should be loaded as text or streamed as readers.
    pub fn with_read_mode(mut self, read_mode: SafeFileReadMode) -> Self {
        self.read_mode = read_mode;
        self
    }

    /// Choose how symlinks should be handled.
    pub fn with_symlink_policy(mut self, symlink_policy: SymlinkPolicy) -> Self {
        self.symlink_policy = symlink_policy;
        self
    }

    /// Resolve a single include request.
    ///
    /// This method is public so callers can use the resolver directly in tests or wrap it in their
    /// own callback logic. For direct integration with [`crate::Options`], use
    /// [`SafeFileResolver::into_callback`].
    pub fn resolve(&self, req: IncludeRequest<'_>) -> Result<ResolvedInclude, IncludeResolveError> {
        let spec_path = Path::new(req.spec);
        validate_relative_include_spec(spec_path, req.spec)?;

        let base_dir = self.base_dir_for_request(&req)?;
        if self.symlink_policy == SymlinkPolicy::Reject {
            self.reject_symlinks_in_spec_path(&base_dir, spec_path, req.spec)?;
        }

        let joined = base_dir.join(spec_path);
        let canonical_target = fs::canonicalize(&joined).map_err(|e| {
            IncludeResolveError::Message(format!(
                "failed to resolve include '{}' from '{}': {}",
                req.spec,
                base_dir.display(),
                e
            ))
        })?;
        self.ensure_inside_root(&canonical_target, req.spec)?;

        let metadata = fs::metadata(&canonical_target)?;
        if !metadata.is_file() {
            return Err(IncludeResolveError::Message(format!(
                "include target '{}' is not a regular file",
                canonical_target.display()
            )));
        }

        let id = path_to_string(&canonical_target);
        if req.from_id.is_none() {
            if let Some(root_source_id) = &self.root_source_id {
                if root_source_id == &id {
                    return Err(IncludeResolveError::Message(format!(
                        "include target '{}' resolves to the configured root file itself",
                        req.spec
                    )));
                }
            }
        }

        let name = display_name(&self.allow_root, &canonical_target);
        let source = match self.read_mode {
            SafeFileReadMode::Text => {
                InputSource::from_string(read_decoded_file(&canonical_target)?)
            }
            SafeFileReadMode::Reader => {
                InputSource::from_reader(fs::File::open(&canonical_target)?)
            }
        };

        Ok(ResolvedInclude { id, name, source })
    }

    /// Convert this resolver into a callback accepted by [`crate::Options::with_include_resolver`].
    pub fn into_callback(
        self,
    ) -> impl for<'req> FnMut(IncludeRequest<'req>) -> Result<ResolvedInclude, IncludeResolveError>
    {
        move |req| self.resolve(req)
    }

    fn base_dir_for_request(
        &self,
        req: &IncludeRequest<'_>,
    ) -> Result<PathBuf, IncludeResolveError> {
        let Some(from_id) = req.from_id else {
            return Ok(self.root_base_dir.clone());
        };

        let from_id_path = Path::new(from_id);
        if !from_id_path.is_absolute() {
            return Err(IncludeResolveError::Message(format!(
                "SafeFileResolver expected parent include id to be an absolute canonical path, got '{}'",
                from_id
            )));
        }

        let from_path = fs::canonicalize(from_id_path).map_err(|e| {
            IncludeResolveError::Message(format!(
                "failed to resolve parent include source '{}' (from '{}'): {}",
                from_id, req.from_name, e
            ))
        })?;
        self.ensure_inside_root(&from_path, req.spec)?;

        let metadata = fs::metadata(&from_path)?;
        if !metadata.is_file() {
            return Err(IncludeResolveError::Message(format!(
                "include parent '{}' is not a regular file",
                from_path.display()
            )));
        }

        let Some(parent) = from_path.parent() else {
            return Err(IncludeResolveError::Message(format!(
                "include parent '{}' does not have a parent directory",
                from_path.display()
            )));
        };

        Ok(parent.to_path_buf())
    }

    fn ensure_inside_root(
        &self,
        canonical_path: &Path,
        spec: &str,
    ) -> Result<(), IncludeResolveError> {
        if canonical_path.starts_with(&self.allow_root) {
            Ok(())
        } else {
            Err(IncludeResolveError::Message(format!(
                "include '{}' resolves outside the configured root '{}'",
                spec,
                self.allow_root.display()
            )))
        }
    }

    fn reject_symlinks_in_spec_path(
        &self,
        base_dir: &Path,
        spec_path: &Path,
        spec_display: &str,
    ) -> Result<(), IncludeResolveError> {
        let mut current = base_dir.to_path_buf();

        for component in spec_path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    current.pop();
                    if !current.starts_with(&self.allow_root) {
                        return Err(IncludeResolveError::Message(format!(
                            "include '{}' resolves outside the configured root '{}'",
                            spec_display,
                            self.allow_root.display()
                        )));
                    }
                }
                Component::Normal(part) => {
                    current.push(part);
                    match fs::symlink_metadata(&current) {
                        Ok(meta) if meta.file_type().is_symlink() => {
                            return Err(IncludeResolveError::Message(format!(
                                "include '{}' traverses a symlink, which is disabled by policy",
                                spec_display
                            )));
                        }
                        Ok(_) => {}
                        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                        Err(err) => return Err(err.into()),
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(IncludeResolveError::Message(format!(
                        "absolute include paths are not allowed: {}",
                        spec_display
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(feature = "include")]
fn read_decoded_file(path: &Path) -> Result<String, IncludeResolveError> {
    let file = fs::File::open(path)?;
    let mut decoder = DecodeReaderBytesBuilder::new()
        .encoding(None)
        .bom_override(true)
        .build(file);
    let mut text = String::new();
    decoder.read_to_string(&mut text)?;
    Ok(text)
}

#[cfg(feature = "include")]
fn canonicalize_existing_dir(path: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::metadata(&canonical)?;
    if metadata.is_dir() {
        Ok(canonical)
    } else {
        Err(invalid_input(format!(
            "expected a directory, got '{}'",
            canonical.display()
        )))
    }
}

#[cfg(feature = "include")]
fn canonicalize_existing_file(path: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::metadata(&canonical)?;
    if metadata.is_file() {
        Ok(canonical)
    } else {
        Err(invalid_input(format!(
            "expected a file, got '{}'",
            canonical.display()
        )))
    }
}

#[cfg(feature = "include")]
fn validate_relative_include_spec(
    spec_path: &Path,
    raw_spec: &str,
) -> Result<(), IncludeResolveError> {
    if raw_spec.is_empty() {
        return Err(IncludeResolveError::Message(
            "include path must not be empty".to_string(),
        ));
    }

    if raw_spec.contains('#') {
        return Err(IncludeResolveError::Message(
            "SafeFileResolver does not support include fragments ('#'); use a custom resolver for fragment-aware includes"
                .to_string(),
        ));
    }

    if spec_path.is_absolute() {
        return Err(IncludeResolveError::Message(format!(
            "absolute include paths are not allowed: {}",
            raw_spec
        )));
    }

    if spec_path
        .components()
        .any(|component| matches!(component, Component::RootDir | Component::Prefix(_)))
    {
        return Err(IncludeResolveError::Message(format!(
            "absolute include paths are not allowed: {}",
            raw_spec
        )));
    }

    Ok(())
}

#[cfg(feature = "include")]
fn display_name(allow_root: &Path, canonical_target: &Path) -> String {
    canonical_target
        .strip_prefix(allow_root)
        .ok()
        .and_then(|relative| {
            if relative.as_os_str().is_empty() {
                None
            } else {
                Some(relative.display().to_string())
            }
        })
        .unwrap_or_else(|| canonical_target.display().to_string())
}

#[cfg(feature = "include")]
fn ensure_inside_root_io(allow_root: &Path, canonical_path: &Path, what: &str) -> io::Result<()> {
    if canonical_path.starts_with(allow_root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} '{}' is outside the configured root '{}'",
                what,
                canonical_path.display(),
                allow_root.display()
            ),
        ))
    }
}

#[cfg(feature = "include")]
fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(feature = "include")]
fn path_to_string(path: &Path) -> String {
    path.as_os_str().to_string_lossy().into_owned()
}