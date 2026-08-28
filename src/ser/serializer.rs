mod compound;
mod helpers;

#[doc(hidden)]
pub use self::compound::{MapSer, SeqSer, StructVariantSer, TupleSer};

use self::helpers::{StrCapture, scalar_key_to_string};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use nohash_hasher::BuildNoHashHasher;
use serde_core::ser::Error as _;
use serde_core::ser::{Serialize, Serializer};
use std::collections::HashMap;
use std::fmt::Write;

use crate::long_strings::{NAME_FOLD_STR, NAME_LIT_STR};
use crate::tag::{YAML_TAG_NAMESPACE, simple_enum_variant_name};

use super::options::{CommentPosition, SerializerOptions};
use super::quoting::{
    escape_double_quoted, is_auto_block_scalar_readable, is_block_scalar_content_safe,
    is_controll_which_needs_escaping, is_plain_value_safe,
};
use super::{
    Error, NAME_DOUBLE_QUOTED, NAME_FLOW_MAP, NAME_FLOW_SEQ, NAME_NULLABLE_TILDE,
    NAME_SINGLE_QUOTED, NAME_SPACE_AFTER, NAME_TUPLE_ANCHOR, NAME_TUPLE_COMMENTED,
    NAME_TUPLE_TAGGED, NAME_TUPLE_WEAK, Result, checked_depth_add, checked_indentation, wrapping,
    zmij_format,
};

// ------------------------------------------------------------
// Core serializer
// ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingFlow {
    AnySeq,
    AnyMap,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum StrStyle {
    Literal, // |
    Folded,  // >
}

/// Block-scalar style request waiting to be consumed by the next string.
///
/// Wrapper types create `Explicit` requests, while the serializer's readability
/// heuristics create `Automatic` requests. Keeping the origin together with the
/// style prevents a stale "automatic" flag from existing when no style is pending.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingStrStyle {
    Explicit(StrStyle),
    Automatic(StrStyle),
}

impl PendingStrStyle {
    /// Return the requested style and whether it came from automatic selection.
    fn into_parts(self) -> (StrStyle, bool) {
        match self {
            Self::Explicit(style) => (style, false),
            Self::Automatic(style) => (style, true),
        }
    }
}

// Numeric anchor id used internally.
type AnchorId = u32;

const MAX_ANCHOR_NAME_BYTES: usize = 256;

/// A tag staged by a wrapper or by an internal serializer feature.
///
/// Equality and anchor compatibility use the resolved identity. `emitted` is
/// only a presentation choice, retained so existing `!!binary` and tagged-enum
/// output stays compact when those features stage the tag first.
struct PendingTag {
    resolved: String,
    emitted: String,
}

#[inline]
const fn is_global_tag_uri_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'#'
                | b';'
                | b'/'
                | b'?'
                | b':'
                | b'@'
                | b'&'
                | b'='
                | b'+'
                | b'$'
                | b','
                | b'_'
                | b'.'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'('
                | b')'
        )
}

#[inline]
const fn is_local_tag_suffix_byte(byte: u8) -> bool {
    is_global_tag_uri_byte(byte) && !matches!(byte, b'!' | b',')
}

fn push_percent_encoded_byte(out: &mut String, byte: u8, safe: bool) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    // A literal '%' must always be escaped: the YAML parser interprets it as
    // the start of an encoded UTF-8 scalar and decodes it again.
    if byte != b'%' && safe {
        out.push(char::from(byte));
    } else {
        out.push('%');
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

fn push_percent_encoded(out: &mut String, value: &str, safe: fn(u8) -> bool) {
    for byte in value.bytes() {
        push_percent_encoded_byte(out, byte, safe(byte));
    }
}

/// Whether `host_and_port` is an IP-literal with an optional numeric port.
fn uri_ip_literal_host_is_valid(host_and_port: &str) -> bool {
    let Some(literal_and_port) = host_and_port.strip_prefix('[') else {
        return false;
    };
    let Some(close) = literal_and_port.find(']') else {
        return false;
    };
    if !uri_ip_literal_is_valid(&literal_and_port[..close]) {
        return false;
    }
    let port = &literal_and_port[close + 1..];
    port.is_empty()
        || port
            .strip_prefix(':')
            .is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn push_uri_authority_percent_encoded(out: &mut String, authority: &str) {
    let host_and_port = if let Some((userinfo, host_and_port)) = authority.rsplit_once('@') {
        push_percent_encoded(out, userinfo, is_uri_userinfo_char);
        out.push('@');
        host_and_port
    } else {
        authority
    };

    if uri_ip_literal_host_is_valid(host_and_port) {
        out.push_str(host_and_port);
        return;
    }

    let (host, port) = match host_and_port.rsplit_once(':') {
        Some((host, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => (host, Some(port)),
        _ => (host_and_port, None),
    };
    push_percent_encoded(out, host, is_uri_reg_name_char);
    if let Some(port) = port {
        out.push(':');
        out.push_str(port);
    }
}

fn push_uri_hier_part_percent_encoded(out: &mut String, hier_part: &str) {
    if let Some(authority_and_path) = hier_part.strip_prefix("//") {
        out.push_str("//");
        let path_start = authority_and_path.find('/');
        let (authority, path) = match path_start {
            Some(idx) => authority_and_path.split_at(idx),
            None => (authority_and_path, ""),
        };
        push_uri_authority_percent_encoded(out, authority);
        push_percent_encoded(out, path, is_uri_path_char);
    } else {
        push_percent_encoded(out, hier_part, is_uri_path_char);
    }
}

fn push_global_tag_uri_percent_encoded(out: &mut String, uri: &str) {
    let Some((scheme, remainder)) = uri.split_once(':') else {
        push_percent_encoded(out, uri, is_global_tag_uri_byte);
        return;
    };
    push_percent_encoded(out, scheme, is_uri_scheme_char);
    out.push(':');

    let (without_fragment, fragment) = match remainder.split_once('#') {
        Some((head, tail)) => (head, Some(tail)),
        None => (remainder, None),
    };
    let (hier_part, query) = match without_fragment.split_once('?') {
        Some((head, tail)) => (head, Some(tail)),
        None => (without_fragment, None),
    };

    push_uri_hier_part_percent_encoded(out, hier_part);
    if let Some(query) = query {
        out.push('?');
        push_percent_encoded(out, query, is_uri_query_or_fragment_char);
    }
    if let Some(fragment) = fragment {
        out.push('#');
        push_percent_encoded(out, fragment, is_uri_query_or_fragment_char);
    }
}

#[inline]
const fn is_uri_scheme_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
}

/// Convert a resolved tag identity into one canonical, injection-safe YAML token.
fn resolved_tag_token(resolved: &str) -> String {
    if let Some(suffix) = resolved.strip_prefix('!') {
        let mut token = String::with_capacity(resolved.len());
        token.push('!');
        push_percent_encoded(&mut token, suffix, is_local_tag_suffix_byte);
        token
    } else {
        let mut token = String::with_capacity(resolved.len() + 3);
        token.push_str("!<");
        push_global_tag_uri_percent_encoded(&mut token, resolved);
        token.push('>');
        token
    }
}

fn core_tag_token(suffix: &str) -> String {
    let mut token = String::with_capacity(suffix.len() + 2);
    token.push_str("!!");
    push_percent_encoded(&mut token, suffix, is_local_tag_suffix_byte);
    token
}

#[inline]
const fn is_uri_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

#[inline]
const fn is_uri_sub_delim(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}

#[inline]
const fn is_uri_pchar(byte: u8) -> bool {
    is_uri_unreserved(byte) || is_uri_sub_delim(byte) || matches!(byte, b':' | b'@')
}

fn uri_component_is_valid(value: &str, allowed: fn(u8) -> bool) -> bool {
    let bytes = value.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%' {
            if idx + 2 >= bytes.len()
                || !bytes[idx + 1].is_ascii_hexdigit()
                || !bytes[idx + 2].is_ascii_hexdigit()
            {
                return false;
            }
            idx += 3;
        } else if allowed(bytes[idx]) {
            idx += 1;
        } else {
            return false;
        }
    }
    true
}

#[inline]
const fn is_uri_userinfo_char(byte: u8) -> bool {
    is_uri_unreserved(byte) || is_uri_sub_delim(byte) || byte == b':'
}

#[inline]
const fn is_uri_reg_name_char(byte: u8) -> bool {
    is_uri_unreserved(byte) || is_uri_sub_delim(byte)
}

#[inline]
const fn is_uri_path_char(byte: u8) -> bool {
    is_uri_pchar(byte) || byte == b'/'
}

#[inline]
const fn is_uri_query_or_fragment_char(byte: u8) -> bool {
    is_uri_pchar(byte) || matches!(byte, b'/' | b'?')
}

#[inline]
const fn is_ipv_future_char(byte: u8) -> bool {
    is_uri_unreserved(byte) || is_uri_sub_delim(byte) || byte == b':'
}

fn uri_ip_literal_is_valid(value: &str) -> bool {
    if value.parse::<std::net::Ipv6Addr>().is_ok() {
        return true;
    }

    let Some(version_and_address) = value.strip_prefix('v').or_else(|| value.strip_prefix('V'))
    else {
        return false;
    };
    let Some((version, address)) = version_and_address.split_once('.') else {
        return false;
    };
    !version.is_empty()
        && version.bytes().all(|byte| byte.is_ascii_hexdigit())
        && !address.is_empty()
        && address.bytes().all(is_ipv_future_char)
}

fn uri_authority_is_valid(authority: &str) -> bool {
    let mut userinfo_and_host = authority.split('@');
    let first = userinfo_and_host.next().unwrap_or_default();
    let (userinfo, host_and_port) = match userinfo_and_host.next() {
        Some(host) if userinfo_and_host.next().is_none() => (Some(first), host),
        Some(_) => return false,
        None => (None, first),
    };
    if userinfo.is_some_and(|value| !uri_component_is_valid(value, is_uri_userinfo_char)) {
        return false;
    }

    if let Some(literal_and_port) = host_and_port.strip_prefix('[') {
        let Some((literal, port)) = literal_and_port.split_once(']') else {
            return false;
        };
        if !uri_ip_literal_is_valid(literal) {
            return false;
        }
        return port.is_empty()
            || port
                .strip_prefix(':')
                .is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_digit()));
    }

    let (host, port) = match host_and_port.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port)),
        Some(_) => return false,
        None => (host_and_port, None),
    };
    uri_component_is_valid(host, is_uri_reg_name_char)
        && port.is_none_or(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Validate the RFC 3986 `URI` production with an explicit scheme.
///
/// Unlike the RFC's narrower `absolute-URI` production, YAML global tags may
/// include a fragment, so this accepts the optional `#fragment` from `URI`.
fn absolute_uri_is_valid(uri: &str) -> bool {
    let Some((scheme, remainder)) = uri.split_once(':') else {
        return false;
    };
    if !scheme
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return false;
    }

    let (without_fragment, fragment) = match remainder.split_once('#') {
        Some((head, tail)) => (head, Some(tail)),
        None => (remainder, None),
    };
    if fragment.is_some_and(|value| !uri_component_is_valid(value, is_uri_query_or_fragment_char)) {
        return false;
    }

    let (hier_part, query) = match without_fragment.split_once('?') {
        Some((head, tail)) => (head, Some(tail)),
        None => (without_fragment, None),
    };
    if query.is_some_and(|value| !uri_component_is_valid(value, is_uri_query_or_fragment_char)) {
        return false;
    }

    if let Some(authority_and_path) = hier_part.strip_prefix("//") {
        let path_start = authority_and_path.find('/');
        let (authority, path) = match path_start {
            Some(idx) => authority_and_path.split_at(idx),
            None => (authority_and_path, ""),
        };
        uri_authority_is_valid(authority) && uri_component_is_valid(path, is_uri_path_char)
    } else {
        uri_component_is_valid(hier_part, is_uri_path_char)
    }
}

fn is_supported_anchor_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_ANCHOR_NAME_BYTES
        && name.chars().all(|ch| {
            !ch.is_whitespace() && !ch.is_control() && !matches!(ch, '[' | ']' | '{' | '}' | ',')
        })
}

/// Immutable formatting policy copied from the public [`SerializerOptions`].
///
/// A serializer captures these values once during construction and never mutates
/// them. Keeping policy separate from [`SerializerState`] makes it clear that
/// recursive serialization may change traversal state but not caller-selected
/// behavior. The boolean fields are independent user-facing switches rather than
/// coupled states in a state machine.
#[allow(clippy::struct_excessive_bools)]
struct SerializerSettings {
    /// Spaces per indentation level for block-style collections.
    indent_step: usize,
    /// Threshold for downgrading block-string wrappers to plain scalars.
    min_fold_chars: usize,
    /// Wrap width for folded block scalars (`>`).
    folded_wrap_col: usize,
    /// Placement mode for [`crate::Commented`] wrappers in block style.
    comment_position: CommentPosition,
    /// Emit YAML tags for simple enums that serialize to a single scalar.
    tagged_enums: bool,
    /// Emit empty maps as `{}` and empty lists as `[]`.
    empty_as_braces: bool,
    /// Emit list items with compact indentation under mapping keys.
    compact_list_indent: bool,
    /// Automatically prefer block scalars for eligible strings.
    prefer_block_scalars: bool,
    /// Quote all string scalars.
    quote_all: bool,
    /// Emit a YAML 1.2 directive and use YAML 1.2-friendly heuristics.
    yaml_12: bool,
}

impl From<&SerializerOptions> for SerializerSettings {
    fn from(options: &SerializerOptions) -> Self {
        Self {
            indent_step: options.indent_step,
            min_fold_chars: options.min_fold_chars,
            folded_wrap_col: options.folded_wrap_chars,
            comment_position: options.comment_position,
            tagged_enums: options.tagged_enums,
            empty_as_braces: options.empty_as_braces,
            compact_list_indent: options.compact_list_indent,
            prefer_block_scalars: options.prefer_block_scalars,
            quote_all: options.quote_all,
            yaml_12: options.yaml_12,
        }
    }
}

/// One-shot layout signals passed between a parent serializer and the nested
/// collection serializer to which it delegates.
///
/// These flags describe pending output work, not durable formatting policy. A
/// consumer must clear a signal after handling it, and code that temporarily
/// overrides a signal while serializing a child must restore the parent value.
#[derive(Default)]
struct PendingLayout {
    /// Emit the first key of the next mapping inline.
    pending_inline_map: bool,
    /// Defer the space following a mapping colon until the value shape is known.
    pending_space_after_colon: bool,
    /// Indent a sequence following an inline-after-dash mapping by one level.
    inline_map_after_dash: bool,
}

/// Mutable traversal and output state for one serializer instance.
///
/// Serde recursively hands the same serializer to child values, so indentation,
/// cursor position, flow depth, and pending node decorations must survive those
/// calls. A fresh state is created for every `YamlSerializer`; none of these
/// values are copied back into the public [`SerializerOptions`].
struct SerializerState {
    /// Current nesting depth used for indentation.
    depth: usize,
    /// Whether the output cursor is at the start of a line.
    at_line_start: bool,
    /// Pending flow-style hint captured from wrapper types.
    pending_flow: Option<PendingFlow>,
    /// Number of nested flow containers currently being emitted.
    in_flow: usize,
    /// Pending explicit or automatically selected block-string style.
    pending_str_style: Option<PendingStrStyle>,
    /// Tag staged for the next node by a wrapper or a serializer feature.
    pending_tag: Option<PendingTag>,
    /// Inline comment waiting for the next scalar.
    pending_inline_comment: Option<String>,
    /// Short-lived layout signals shared by nested collection serializers.
    pending_layout: PendingLayout,
    /// Whether the last serialized value was a block collection.
    last_value_was_block: bool,
    /// Depth captured for the node immediately following a sequence dash.
    after_dash_depth: Option<usize>,
    /// Current block-map depth for aligning sequences below mapping keys.
    current_map_depth: Option<usize>,
    /// Whether emission of the current document has begun.
    doc_started: bool,
}

impl Default for SerializerState {
    fn default() -> Self {
        Self {
            depth: 0,
            at_line_start: true,
            pending_flow: None,
            in_flow: 0,
            pending_str_style: None,
            pending_tag: None,
            pending_inline_comment: None,
            pending_layout: PendingLayout::default(),
            last_value_was_block: false,
            after_dash_depth: None,
            current_map_depth: None,
            doc_started: false,
        }
    }
}

/// Per-serializer registry for YAML anchors and aliases.
///
/// Pointer identities receive stable, monotonically increasing ids. When a new
/// identity is discovered, `pending_id` defers writing its `&anchor` prefix until
/// the value's serializer reveals whether the next node is scalar or compound.
/// Custom names are cached by id so later aliases reuse exactly the same name.
struct AnchorState {
    /// Map from pointer identity to anchor id.
    by_ptr: HashMap<usize, AnchorId, BuildNoHashHasher<usize>>,
    /// Next numeric id to use when generating anchor names.
    next_id: AnchorId,
    /// Anchor to prefix onto the next scalar or complex node.
    pending_id: Option<AnchorId>,
    /// Optional custom anchor-name generator supplied by the caller.
    generator: Option<fn(usize) -> String>,
    /// Cached custom names, indexed by `id - 1`.
    custom_names: Option<Vec<String>>,
    /// Resolved tag attached to each anchor definition, indexed by `id - 1`.
    resolved_tags: Vec<Option<String>>,
}

impl AnchorState {
    fn new(generator: Option<fn(usize) -> String>) -> Self {
        Self {
            by_ptr: HashMap::with_hasher(BuildNoHashHasher::default()),
            next_id: 1,
            pending_id: None,
            generator,
            custom_names: None,
            resolved_tags: Vec::new(),
        }
    }

    fn record_resolved_tag(&mut self, id: AnchorId, tag: Option<&str>) {
        let idx = id as usize - 1;
        if self.resolved_tags.len() <= idx {
            self.resolved_tags.resize_with(idx + 1, || None);
        }
        self.resolved_tags[idx] = tag.map(ToOwned::to_owned);
    }

    fn resolved_tag(&self, id: AnchorId) -> Option<&str> {
        self.resolved_tags
            .get(id as usize - 1)
            .and_then(Option::as_deref)
    }
}

/// Core YAML serializer used by `to_string`, `to_fmt_writer`, and `to_io_writer` (and their `_with_options` variants).
///
/// This type implements `serde::Serializer` and writes YAML to a `fmt::Write`.
/// It manages indentation, flow/block styles, and YAML anchors/aliases.
///
/// This type is also re-exported from the crate root as [`serde_saphyr::Serializer`](crate::Serializer).
///
/// ## Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Foo {
///     a: i32,
///     b: bool,
/// }
///
/// let mut out = String::new();
/// let mut ser = serde_saphyr::Serializer::new(&mut out);
/// Foo { a: 1, b: true }.serialize(&mut ser)?;
///
/// assert!(out.contains("a: 1"));
/// # Ok::<(), serde_saphyr::ser::Error>(())
/// ```
pub struct YamlSerializer<'a, W: Write> {
    /// Destination writer where YAML text is emitted.
    out: &'a mut W,
    /// Immutable caller-selected formatting behavior.
    settings: SerializerSettings,
    /// Mutable layout and traversal state shared with nested serializers.
    state: SerializerState,
    /// Anchor identities, names, and deferred anchor emission.
    anchors: AnchorState,
}

impl<'a, W: Write> YamlSerializer<'a, W> {
    /// Construct a [`Serializer`](crate::Serializer) that writes to `out`.
    /// Uses the same defaults as `SerializerOptions::default()`.
    #[must_use]
    pub fn new(out: &'a mut W) -> Self {
        Self::from_options_unchecked(out, &SerializerOptions::default())
    }

    fn from_options_unchecked(out: &'a mut W, options: &SerializerOptions) -> Self {
        Self {
            out,
            settings: SerializerSettings::from(options),
            state: SerializerState::default(),
            anchors: AnchorState::new(options.anchor_generator),
        }
    }
    /// Construct a [`Serializer`](crate::Serializer) with a specific indentation step.
    /// All other settings use `SerializerOptions::default()`. Returns an error if
    /// `indent_step` is outside `1..=64`.
    pub fn with_indent(out: &'a mut W, indent_step: usize) -> Result<Self> {
        let options = SerializerOptions {
            indent_step,
            ..SerializerOptions::default()
        };
        Self::with_options(out, options)
    }
    /// Construct a [`Serializer`](crate::Serializer) from user-supplied [`SerializerOptions`].
    /// Used by `to_fmt_writer_with_options` and `to_io_writer_with_options`.
    pub fn with_options(out: &'a mut W, options: SerializerOptions) -> Result<Self> {
        options.consistent()?;
        Ok(Self::from_options_unchecked(out, &options))
    }

    // -------- helpers --------

    /// Determines if a string requires double quotes when `quote_all` is enabled.
    /// Returns true if the string contains single quotes, backslashes, or control characters
    /// that need escape processing.
    #[inline]
    fn needs_double_quotes(s: &str) -> bool {
        s.chars().any(|c| {
            c == '\''       // single quote present - cannot use single-quoted style
                || c == '\\' // backslash - needs escape processing
                || is_controll_which_needs_escaping(c) // control chars (includes \n, \t, \r, etc.) need escaping
        })
    }

    /// Write a single-quoted string. Single quotes inside the string are escaped by doubling them.
    fn write_single_quoted(&mut self, s: &str) -> Result<()> {
        self.out.write_char('\'')?;
        for ch in s.chars() {
            if ch == '\'' {
                self.out.write_str("''")?; // escape single quote by doubling
            } else {
                self.out.write_char(ch)?;
            }
        }
        self.out.write_char('\'')?;
        Ok(())
    }

    /// Append a pending inline comment, if any.
    ///
    /// Used both by normal scalar emission (`value # comment\n`) and by
    /// block-scalar headers (`| # comment\n`). Comments are suppressed in flow
    /// style, matching the existing serializer policy.
    #[inline]
    fn write_pending_inline_comment(&mut self) -> Result<()> {
        if self.state.in_flow == 0
            && let Some(c) = self.state.pending_inline_comment.take()
        {
            self.out.write_str(" # ")?;
            self.out.write_str(&c)?;
        }
        Ok(())
    }

    /// Called at the end of emitting a scalar in block style: appends a pending inline
    /// comment (if any) and then emits a newline. In flow style, comments are suppressed.
    #[inline]
    fn write_end_of_scalar(&mut self) -> Result<()> {
        if self.state.in_flow == 0 {
            self.write_pending_inline_comment()?;
            self.newline()?;
        }
        Ok(())
    }

    #[inline]
    fn sanitize_comment_text(comment: &str) -> String {
        comment
            .chars()
            .map(|ch| match ch {
                '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}' => ' ',
                _ => ch,
            })
            .collect()
    }

    fn write_above_comment(&mut self, comment: &str) -> Result<Option<usize>> {
        if self.state.in_flow > 0 || comment.is_empty() {
            return Ok(None);
        }

        let target_depth = if self.state.pending_layout.pending_space_after_colon {
            let base = self.state.current_map_depth.unwrap_or(self.state.depth);
            self.state.pending_layout.pending_space_after_colon = false;
            if !self.state.at_line_start {
                self.newline()?;
            }
            checked_depth_add(base, 1)?
        } else if !self.state.at_line_start {
            let base = self.state.after_dash_depth.unwrap_or(self.state.depth);
            self.state.pending_layout.pending_inline_map = false;
            self.newline()?;
            checked_depth_add(base, 1)?
        } else {
            self.state.depth
        };

        self.write_indent(target_depth)?;
        self.out.write_str("# ")?;
        self.out.write_str(comment)?;
        self.newline()?;
        Ok(Some(target_depth))
    }

    /// Allocate (or get existing) anchor id for a pointer identity.
    /// Returns `(id, is_new)`, or an error when the custom generator returns an unsupported name.
    #[inline]
    fn alloc_anchor_for(&mut self, ptr: usize) -> Result<(AnchorId, bool)> {
        match self.anchors.by_ptr.entry(ptr) {
            std::collections::hash_map::Entry::Occupied(e) => Ok((*e.get(), false)),
            std::collections::hash_map::Entry::Vacant(v) => {
                let id = self.anchors.next_id;
                let custom_name = if let Some(generator) = self.anchors.generator {
                    let name = generator(id as usize);
                    if !is_supported_anchor_name(&name) {
                        return Err(Error::InvalidOptions(format!(
                            "custom anchor generator returned an invalid name for anchor id {id}; names must be 1..={MAX_ANCHOR_NAME_BYTES} bytes and contain no whitespace, control characters, or '[', ']', '{{', '}}', ','"
                        )));
                    }
                    Some(name)
                } else {
                    None
                };

                self.anchors.next_id = self.anchors.next_id.saturating_add(1);
                if let Some(name) = custom_name {
                    self.anchors
                        .custom_names
                        .get_or_insert_with(Vec::new)
                        .push(name);
                }
                v.insert(id);
                Ok((id, true))
            }
        }
    }

    /// Resolve an anchor name for `id` and write it.
    #[inline]
    fn write_anchor_name(&mut self, id: AnchorId) -> Result<()> {
        if let Some(names) = &self.anchors.custom_names {
            // ids are 1-based; vec is 0-based
            let idx = id as usize - 1;
            if let Some(name) = names.get(idx) {
                self.out.write_str(name)?;
            } else {
                // Fallback if generator vector is out of sync
                write!(self.out, "a{id}")?;
            }
        } else {
            write!(self.out, "a{id}")?;
        }
        Ok(())
    }

    /// If a mapping key has just been written (':' emitted) and we determined the value is a scalar,
    /// insert a single space before the scalar and clear the pending flag.
    #[inline]
    fn write_space_if_pending(&mut self) -> Result<()> {
        if self.state.pending_layout.pending_space_after_colon {
            self.out.write_char(' ')?;
            self.state.pending_layout.pending_space_after_colon = false;
        }
        // When a scalar value is serialized, it should reset the block-sibling flag.
        // Most scalar emitters call this method.
        self.state.last_value_was_block = false;
        Ok(())
    }

    /// Ensure indentation is written if we are at the start of a line.
    /// Internal: called by most emitters before writing tokens.
    #[inline]
    fn write_indent(&mut self, depth: usize) -> Result<()> {
        if self.state.at_line_start {
            if !self.state.doc_started {
                self.state.doc_started = true;
                if self.settings.yaml_12 {
                    self.out.write_str("%YAML 1.2\n---\n")?;
                    // Still at start of a line after the directive and document start marker.
                    self.state.at_line_start = true;
                }
            }
            self.write_indent_spaces(depth)?;
            self.state.at_line_start = false;
        }
        Ok(())
    }

    #[inline]
    fn write_indent_spaces(&mut self, depth: usize) -> Result<()> {
        let spaces = checked_indentation(self.settings.indent_step, depth)?;
        for _ in 0..spaces {
            self.out.write_char(' ')?;
        }
        Ok(())
    }

    /// Emit a newline and mark the next write position as line start.
    /// Internal utility used after finishing a top-level token.
    #[inline]
    fn newline(&mut self) -> Result<()> {
        self.out.write_char('\n')?;
        self.state.at_line_start = true;
        Ok(())
    }

    /// Write a folded block string body, wrapping to `folded_wrap_col` characters.
    /// Delegates to the standalone function in `wrapping` module.
    fn write_folded_block(&mut self, s: &str, indent: usize) -> Result<()> {
        self::wrapping::write_folded_block(
            self.out,
            s,
            indent,
            self.settings.indent_step,
            self.settings.folded_wrap_col,
        )?;
        self.state.at_line_start = true;
        Ok(())
    }

    /// Write a scalar in mapping-key position.
    ///
    /// Externally tagged enum variants are emitted as YAML mapping keys
    /// (`Variant: ...`), so they need the same ambiguity checks as regular
    /// map and struct keys.
    fn write_key_scalar(&mut self, s: &str) -> Result<()> {
        let text = scalar_key_to_string(&s, self.settings.yaml_12)?;
        self.out.write_str(&text)?;
        Ok(())
    }

    /// Write a double-quoted string with necessary escapes.
    fn write_quoted(&mut self, s: &str) -> Result<()> {
        self.out.write_char('"')?;
        escape_double_quoted(s, self.out)?;
        self.out.write_char('"')?;
        Ok(())
    }

    /// Like `write_plain_or_quoted`, but intended for VALUE position where ':' is allowed.
    #[inline]
    fn write_plain_or_quoted_value(&mut self, s: &str) -> Result<()> {
        if self.settings.quote_all {
            // In quote_all mode: prefer single quotes, use double quotes when needed
            if Self::needs_double_quotes(s) {
                self.write_quoted(s)
            } else {
                self.write_single_quoted(s)
            }
        } else if is_plain_value_safe(s, self.settings.yaml_12, self.state.in_flow > 0) {
            self.out.write_str(s)?;
            Ok(())
        } else {
            // Force quoted style for problematic value tokens (commas/brackets, bool/num-like, etc.).
            self.write_quoted(s)
        }
    }

    /// Stage a resolved tag supplied by [`crate::Tagged`].
    fn stage_resolved_tag(&mut self, resolved: &str) -> Result<()> {
        if resolved.is_empty() {
            return Err(Error::EmptyResolvedTag);
        }
        let emitted = resolved_tag_token(resolved);
        if !resolved.starts_with('!') && !absolute_uri_is_valid(&emitted[2..emitted.len() - 1]) {
            return Err(Error::InvalidGlobalTagUri {
                tag: resolved.to_owned(),
            });
        }
        self.stage_tag_with_token(resolved, emitted)
    }

    /// Stage a tag with a preferred spelling used by built-in serializer
    /// features. A YAML node carries at most one tag, so tags that resolve to
    /// the same identity coalesce and are emitted once; only two different
    /// identities competing for one node are a clash.
    fn stage_tag_with_token(&mut self, resolved: &str, emitted: String) -> Result<()> {
        match self.state.pending_tag.as_ref() {
            Some(pending) if pending.resolved == resolved => Ok(()),
            Some(pending) => Err(Error::custom(format_args!(
                "cannot serialize one YAML node with both tag {:?} and tag {:?}",
                pending.resolved, resolved
            ))),
            None => {
                self.state.pending_tag = Some(PendingTag {
                    resolved: resolved.to_owned(),
                    emitted,
                });
                Ok(())
            }
        }
    }

    fn stage_core_tag(&mut self, suffix: &str) -> Result<()> {
        let mut resolved = String::with_capacity(YAML_TAG_NAMESPACE.len() + suffix.len());
        resolved.push_str(YAML_TAG_NAMESPACE);
        resolved.push_str(suffix);
        self.stage_tag_with_token(&resolved, core_tag_token(suffix))
    }

    /// Whether the tag staged for the next node is a tag that Serde's enum
    /// deserializer interprets as `variant`.
    #[inline]
    fn pending_tag_selects_variant(&self, variant: &str) -> bool {
        self.state
            .pending_tag
            .as_ref()
            .map(|tag| tag.resolved.as_str())
            .and_then(simple_enum_variant_name)
            == Some(variant)
    }

    /// Serialize a tagged scalar of the form `!!Type value` using plain or quoted style for
    /// the value depending on its content.
    fn serialize_tagged_scalar(&mut self, enum_name: &str, variant: &str) -> Result<()> {
        self.stage_core_tag(enum_name)?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.write_plain_or_quoted_value(variant)?;
        self.write_end_of_scalar()
    }

    fn serialize_double_quoted_scalar(&mut self, value: &str) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.write_quoted(value)?;
        self.write_end_of_scalar()
    }

    fn serialize_single_quoted_scalar(&mut self, value: &str) -> Result<()> {
        if let Some(ch) = value
            .chars()
            .find(|ch| is_controll_which_needs_escaping(*ch))
        {
            return Err(Error::SingleQuotedRequiresEscaping { ch });
        }
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.write_single_quoted(value)?;
        self.write_end_of_scalar()
    }

    fn serialize_tilde_null(&mut self) -> Result<()> {
        self.state.pending_flow = None;
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.out.write_char('~')?;
        self.write_end_of_scalar()
    }

    /// Emit pending YAML node properties before a scalar.
    #[inline]
    fn write_scalar_prefix_if_anchor(&mut self) -> Result<()> {
        let tag = self.state.pending_tag.take();
        let anchor_id = self.anchors.pending_id.take();
        if tag.is_some() || anchor_id.is_some() {
            if self.state.at_line_start {
                self.write_indent(self.state.depth)?;
            }
            if let Some(tag) = tag.as_ref() {
                self.out.write_str(&tag.emitted)?;
                self.out.write_char(' ')?;
            }
            if let Some(id) = anchor_id {
                self.anchors
                    .record_resolved_tag(id, tag.as_ref().map(|tag| tag.resolved.as_str()));
                self.out.write_char('&')?;
                self.write_anchor_name(id)?;
                self.out.write_char(' ')?;
            }
        }
        Ok(())
    }

    /// Emit pending YAML node properties on their own line before a block node.
    #[inline]
    fn write_anchor_for_complex_node(&mut self) -> Result<()> {
        let tag = self.state.pending_tag.take();
        let anchor_id = self.anchors.pending_id.take();
        if tag.is_some() || anchor_id.is_some() {
            self.write_space_if_pending()?;
            if self.state.at_line_start {
                self.write_indent(self.state.depth)?;
            }
            if let Some(tag) = tag.as_ref() {
                self.out.write_str(&tag.emitted)?;
            }
            if let Some(id) = anchor_id {
                self.anchors
                    .record_resolved_tag(id, tag.as_ref().map(|tag| tag.resolved.as_str()));
                if tag.is_some() {
                    self.out.write_char(' ')?;
                }
                self.out.write_char('&')?;
                self.write_anchor_name(id)?;
            }
            self.newline()?;
        }
        Ok(())
    }

    /// Emit an alias `*name`. Adds a newline in block style.
    /// Used when a previously defined anchor is referenced again.
    #[inline]
    fn write_alias_id(&mut self, id: AnchorId) -> Result<()> {
        let anchor_tag = self.anchors.resolved_tag(id);
        if let Some(requested) = self.state.pending_tag.take()
            && anchor_tag != Some(requested.resolved.as_str())
        {
            return Err(Error::custom(format_args!(
                "cannot apply tag {:?} to alias whose anchor was defined with tag {:?}",
                requested.resolved,
                anchor_tag.unwrap_or("")
            )));
        }
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.write_space_if_pending()?;
        self.out.write_char('*')?;
        self.write_anchor_name(id)?;
        // Use the shared end-of-scalar path so pending inline comments are appended in block style
        self.write_end_of_scalar()?;
        Ok(())
    }

    #[inline]
    fn has_pending_node_properties(&self) -> bool {
        self.state.pending_tag.is_some() || self.anchors.pending_id.is_some()
    }

    /// Determine whether the next sequence should be emitted in flow style.
    /// Consumes any pending flow hint.
    #[inline]
    fn take_flow_for_seq(&mut self) -> bool {
        if self.state.in_flow > 0 {
            true
        } else {
            matches!(self.state.pending_flow.take(), Some(PendingFlow::AnySeq))
        }
    }
    /// Determine whether the next mapping should be emitted in flow style.
    /// Consumes any pending flow hint.
    #[inline]
    fn take_flow_for_map(&mut self) -> bool {
        if self.state.in_flow > 0 {
            true
        } else {
            matches!(self.state.pending_flow.take(), Some(PendingFlow::AnyMap))
        }
    }

    /// Temporarily mark that we are inside a flow container while running `f`.
    /// Ensures proper comma insertion and line handling for nested flow nodes.
    #[inline]
    fn with_in_flow<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.state.in_flow += 1;
        let r = f(self);
        self.state.in_flow -= 1;
        r
    }
}

// ------------------------------------------------------------
// Impl Serializer for YamlSerializer
// ------------------------------------------------------------

impl<'a, 'b, W: Write> Serializer for &'a mut YamlSerializer<'b, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSer<'a, 'b, W>;
    type SerializeTuple = SeqSer<'a, 'b, W>;
    type SerializeTupleStruct = TupleSer<'a, 'b, W>;
    type SerializeTupleVariant = SeqSer<'a, 'b, W>;
    type SerializeMap = MapSer<'a, 'b, W>;
    type SerializeStruct = MapSer<'a, 'b, W>;
    type SerializeStructVariant = StructVariantSer<'a, 'b, W>;

    // -------- Scalars --------

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.out.write_str(if v { "true" } else { "false" })?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        write!(self.out, "{v}")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        write!(self.out, "{v}")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        write!(self.out, "{v}")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        write!(self.out, "{v}")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        zmij_format::write_float_string(self.out, v)?;
        self.write_end_of_scalar()
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        zmij_format::write_float_string(self.out, v)?;
        self.write_end_of_scalar()
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.write_space_if_pending()?;
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        #[inline]
        fn block_indent_indicator_digit(indent_n: usize) -> Result<char> {
            // YAML 1.2 8.1.1.1: the block-scalar indentation indicator is `[1-9]`.
            // `char::from_digit` would accept 0, so we gate on the range explicitly.
            match indent_n {
                1..=9 => {
                    let digit = u32::try_from(indent_n).expect("checked 1..=9");
                    Ok(char::from_digit(digit, 10).expect("checked 1..=9"))
                }
                _ => Err(Error::custom(
                    "indentation indicator must be a single digit (1..=9)",
                )),
            }
        }

        // If no explicit style pending, auto-select block style.
        //
        // Controlled by `prefer_block_scalars`:
        //  - multiline → literal (|) whenever the content is representable in a block scalar
        //    and is readable enough to auto-select (see `is_auto_block_scalar_readable`).
        //    Block scalars happily contain ':', '#', YAML-like text, etc. — those are unsafe
        //    in plain style but fine as block content.
        //  - single-line + long (by folded_wrap_col) → folded (>)
        //
        // Also skip block scalars when quote_all is enabled - use quoted strings instead.
        if self.state.pending_str_style.is_none()
            && self.state.in_flow == 0
            && !self.settings.quote_all
        {
            if v.contains('\n') {
                if self.settings.prefer_block_scalars
                    && is_block_scalar_content_safe(v)
                    && is_auto_block_scalar_readable(v)
                {
                    self.state.pending_str_style =
                        Some(PendingStrStyle::Automatic(StrStyle::Literal));
                }
            } else if self.settings.prefer_block_scalars {
                // Single-line string. If it needs quoting as a value, don't auto-fold.
                // Folded block scalars can preserve trailing ASCII spaces, unlike plain
                // scalars, so ignore only those spaces for the eligibility probe.
                let auto_fold_probe = v.trim_end_matches(' ');
                let can_auto_fold = !auto_fold_probe.is_empty()
                    && is_plain_value_safe(auto_fold_probe, self.settings.yaml_12, false);
                if can_auto_fold {
                    // Measure in characters, not bytes.
                    if v.chars().count() > self.settings.folded_wrap_col {
                        self.state.pending_str_style =
                            Some(PendingStrStyle::Automatic(StrStyle::Folded));
                    }
                }
            }
        }
        if let Some(pending_style) = self.state.pending_str_style.take() {
            let (style, from_auto) = pending_style.into_parts();
            if !is_block_scalar_content_safe(v) {
                self.write_space_if_pending()?;
                self.write_scalar_prefix_if_anchor()?;
                if self.state.at_line_start {
                    self.write_indent(self.state.depth)?;
                }
                self.write_quoted(v)?;
                self.write_end_of_scalar()?;
                return Ok(());
            }

            // Emit block string. If we are a mapping value, YAML requires a space after ':'.
            // Insert it now if pending.
            //
            // IMPORTANT: capture whether we were in a map-value position *before* clearing
            // `pending_space_after_colon`, as that context influences indentation.
            let was_map_value = self.state.pending_layout.pending_space_after_colon;
            self.write_space_if_pending()?;
            // Determine base indentation for the block scalar header/body.
            //
            // Important: `after_dash_depth` is only meaningful for the immediate node that
            // follows a sequence dash ("- "). It must NOT affect nested scalars inside a
            // mapping that happens to be a sequence element, otherwise block scalar bodies
            // become under-indented (invalid YAML).
            //
            // For map values (we are mid-line after `key:`), prefer the mapping depth.
            // Otherwise, if we are starting a new node right after a dash, use that depth.
            let base = if was_map_value {
                self.state.current_map_depth.unwrap_or(self.state.depth)
            } else {
                // Use after_dash_depth when available (we're after a sequence dash),
                // regardless of at_line_start (which is false after writing "- ").
                self.state.after_dash_depth.unwrap_or(self.state.depth)
            };
            if self.state.at_line_start {
                self.write_indent(base)?;
            }

            // Anchors/tags are node properties and must be emitted before scalar content.
            // For a block scalar this produces e.g. `text: &a1 |` — without this, a pending
            // anchor would leak onto the next scalar.
            self.write_scalar_prefix_if_anchor()?;

            // Compute the indentation indicator N for block scalars.
            //
            // Per YAML 1.2 8.1.1.1, the indicator is the *additional* indentation steps
            // beyond the parent node's indentation level — NOT the absolute column count.
            // Since our body is exactly one serializer depth (i.e. `indent_step` spaces)
            // deeper than its parent, the indicator is simply `indent_step`.
            //
            // We only emit it when the first non-empty content line has leading whitespace,
            // which would otherwise prevent automatic indentation detection by the parser.
            let body_base = checked_depth_add(base, 1)?;
            let indent_n = self.settings.indent_step;

            // Check if we need an explicit indentation indicator.
            // Required when the first non-empty line has leading whitespace.
            let content_trimmed = v.trim_end_matches('\n');
            let first_line_spaces = self::wrapping::first_line_leading_spaces(content_trimmed);
            let needs_indicator = first_line_spaces > 0;

            // Resolve the indicator digit up front. If the helper rejects `indent_n`
            // (i.e. outside the YAML 1.2 `[1-9]` grammar), fall back to quoting.
            // Anchor prefix is already written above, so don't call it again here.
            let indicator_digit = if needs_indicator {
                let Ok(digit) = block_indent_indicator_digit(indent_n) else {
                    self.write_plain_or_quoted_value(v)?;
                    self.write_end_of_scalar()?;
                    return Ok(());
                };
                Some(digit)
            } else {
                None
            };

            match style {
                StrStyle::Literal => {
                    // Determine trailing newline count to select chomp indicator:
                    //  - 0 → "|-" (strip)
                    //  - 1 → "|" (clip)
                    //  - >=2 → "|+" (keep)
                    let content = v.trim_end_matches('\n');
                    let trailing_nl = v.len() - content.len();

                    // Write block scalar header: | or |N with optional chomp indicator
                    self.out.write_char('|')?;
                    if let Some(digit) = indicator_digit {
                        self.out.write_char(digit)?;
                    }
                    match trailing_nl {
                        0 => self.out.write_char('-')?,
                        1 => {} // clip is the default, no indicator needed
                        _ => self.out.write_char('+')?,
                    }
                    self.write_pending_inline_comment()?;
                    self.newline()?;

                    // Emit body lines. For non-empty content, write each line exactly once.
                    // For keep chomping (>=2), append (trailing_nl - 1) visual empty lines.
                    // Special case: empty original content with at least one trailing newline
                    // should produce a single empty content line (tests expect this for "\n").
                    // Precompute body indent string once for the entire block
                    let mut indent_buf: String = String::new();
                    let spaces = checked_indentation(self.settings.indent_step, body_base)?;
                    if spaces > 0 {
                        indent_buf.reserve(spaces);
                        for _ in 0..spaces {
                            indent_buf.push(' ');
                        }
                    }
                    let indent_str = indent_buf.as_str();

                    if content.is_empty() {
                        if trailing_nl >= 1 {
                            self.out.write_str(indent_str)?;
                            self.state.at_line_start = false;
                            // write a single empty content line
                            self.newline()?;
                        }
                    } else {
                        for line in content.split('\n') {
                            self.out.write_str(indent_str)?;
                            self.state.at_line_start = false;
                            self.out.write_str(line)?;
                            self.newline()?;
                        }
                        if trailing_nl >= 2 {
                            for _ in 0..(trailing_nl - 1) {
                                self.out.write_str(indent_str)?;
                                self.state.at_line_start = false;
                                self.newline()?;
                            }
                        }
                    }
                }
                StrStyle::Folded => {
                    // Write block scalar header: > or >N with optional chomp indicator
                    self.out.write_char('>')?;
                    if let Some(digit) = indicator_digit {
                        self.out.write_char(digit)?;
                    }
                    if from_auto {
                        // Auto-selected folded style: choose chomping based on trailing newlines
                        // to preserve exact content on round-trip.
                        let content = v.trim_end_matches('\n');
                        let trailing_nl = v.len() - content.len();
                        match trailing_nl {
                            0 => self.out.write_char('-')?,
                            1 => {} // clip is the default, no indicator needed
                            _ => self.out.write_char('+')?,
                        }
                    }
                    // Note: Explicit FoldStr/FoldString wrappers historically used plain '>'
                    // regardless of trailing newline; keep that behavior for compatibility.
                    self.write_pending_inline_comment()?;
                    self.newline()?;
                    self.write_folded_block(v, body_base)?;
                }
            }
            return Ok(());
        }
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        // Special-case: prefer single-quoted style for select 1-char punctuation to
        // match expected YAML output in tests ('.', '#', '-').
        if v.len() == 1
            && let Some(ch) = v.chars().next()
            && (ch == '.' || ch == '#' || ch == '-')
        {
            self.out.write_char('\'')?;
            self.out.write_char(ch)?;
            self.out.write_char('\'')?;
            self.write_end_of_scalar()?;
            return Ok(());
        }
        self.write_plain_or_quoted_value(v)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        // Three behaviors are required by tests:
        // - Top-level &[u8] should serialize as a block sequence of integers.
        // - A captured root !!binary tag must keep the bytes on a base64 scalar;
        //   attaching that tag to the usual root sequence changes the YAML node kind.
        // - Fields using #[serde(with = "serde_bytes")] should serialize as a tagged !!binary
        //   base64 scalar inline after "key: ". The latter ends up calling serialize_bytes in
        //   value position (mid-line), whereas plain Vec<u8> without serde_bytes goes through
        //   serialize_seq instead.
        //
        // The untagged sequence is reachable only while no tag is staged at all. Any staged
        // tag makes this a tagged scalar node, so the base64 branch below both keeps a
        // requested !!binary on a scalar and lets any other identity clash with the
        // !!binary that branch stages, exactly as it does in value position.
        if self.state.at_line_start && self.state.pending_tag.is_none() {
            // Top-level or start-of-line: emit as sequence of numbers
            let mut seq = self.serialize_seq(Some(v.len()))?;
            for b in v {
                serde_core::ser::SerializeSeq::serialize_element(&mut seq, b)?;
            }
            return serde_core::ser::SerializeSeq::end(seq);
        }

        // Inline value position, or a root value whose captured tag is binary:
        // emit !!binary with base64.
        self.write_space_if_pending()?;
        self.stage_core_tag("binary")?;
        self.write_scalar_prefix_if_anchor()?;
        // The prefix writer handles root indentation and inline placement.
        let mut s = String::new();
        B64.encode_string(v, &mut s);
        self.out.write_str(&s)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        self.state.last_value_was_block = false;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.out.write_str("null")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        self.state.last_value_was_block = false;
        if self.state.at_line_start {
            self.write_indent(self.state.depth)?;
        }
        self.out.write_str("null")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        // If we are in a mapping value position, insert the deferred space after ':'
        self.write_space_if_pending()?;
        if self.settings.tagged_enums {
            self.serialize_tagged_scalar(name, variant)
        } else {
            self.serialize_str(variant)
        }
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        // Flow hints & block-string hints:
        match name {
            NAME_FLOW_SEQ => {
                self.state.pending_flow = Some(PendingFlow::AnySeq);
                return value.serialize(self);
            }
            NAME_FLOW_MAP => {
                self.state.pending_flow = Some(PendingFlow::AnyMap);
                return value.serialize(self);
            }
            NAME_LIT_STR => {
                // Always use literal block style for LitStr/LitString wrappers.
                // Choose chomping based on trailing newlines during actual emission.
                // Capture the inner string first.
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let s = cap.finish()?;
                self.state.pending_str_style = Some(PendingStrStyle::Explicit(StrStyle::Literal));
                return self.serialize_str(&s);
            }
            NAME_FOLD_STR => {
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let s = cap.finish()?;
                let is_multiline = s.contains('\n');
                if !is_multiline && s.len() < self.settings.min_fold_chars {
                    return self.serialize_str(&s);
                }
                self.state.pending_str_style = Some(PendingStrStyle::Explicit(StrStyle::Folded));
                return self.serialize_str(&s);
            }
            NAME_SPACE_AFTER => {
                // Serialize the value, then emit an empty line after (only in block style).
                let result = value.serialize(&mut *self);
                if self.state.in_flow == 0 {
                    // Emit an extra blank line after the value
                    self.newline()?;
                }
                return result;
            }
            NAME_DOUBLE_QUOTED => {
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let text = cap.finish()?;
                return self.serialize_double_quoted_scalar(&text);
            }
            NAME_SINGLE_QUOTED => {
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let text = cap.finish()?;
                return self.serialize_single_quoted_scalar(&text);
            }
            NAME_NULLABLE_TILDE => {
                return self.serialize_tilde_null();
            }
            _ => {}
        }
        // default: ignore the name, serialize the inner as-is
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        // `!Variant payload` already carries the same variant selection as
        // Serde's usual `{Variant: payload}` representation. Keep the tag
        // staged for the payload and omit the redundant external wrapper.
        if self.pending_tag_selects_variant(variant) {
            return value.serialize(self);
        }

        let was_inline_value = self.state.pending_layout.pending_space_after_colon;
        let anchor_broke_line = self.has_pending_node_properties();
        let after_dash_depth = self.state.after_dash_depth;
        self.write_anchor_for_complex_node()?;

        // If we are the value of a mapping key, YAML forbids "key: Variant: value" inline.
        // Emit the variant mapping on the next line indented one level. Also, do not insert
        // a space after the colon when the value may itself be a mapping; instead, defer
        // space insertion to the value serializer via pending_space_after_colon.
        if was_inline_value {
            // consume the pending space request and start a new line
            self.state.pending_layout.pending_space_after_colon = false;
            if !self.state.at_line_start {
                self.newline()?;
            }
            // When used as a mapping value, indent relative to the parent mapping's base,
            // not the serializer's current depth (which may still be the outer level).
            let base = self.state.current_map_depth.unwrap_or(self.state.depth);
            let variant_depth = checked_depth_add(base, 1)?;
            self.write_indent(variant_depth)?;
            self.write_key_scalar(variant)?;
            // Write ':' without trailing space, then mark that a space may be needed
            // if the following value is a scalar.
            self.out.write_str(":")?;
            self.state.pending_layout.pending_space_after_colon = true;
            self.state.at_line_start = false;
            // Do not let any inline-after-dash hint leak into the variant's inner value.
            // After `Variant:`, the next node is in value position and must choose its own layout.
            self.state.pending_layout.pending_inline_map = false;
            // Ensure that if the value is another variant or a mapping/sequence,
            // it indents under this variant label rather than the parent map key.
            let prev_map_depth = self.state.current_map_depth.replace(variant_depth);
            let res = value.serialize(&mut *self);
            self.state.current_map_depth = prev_map_depth;
            return res;
        }
        // Otherwise (top-level or sequence context).
        if self.state.at_line_start {
            let depth = if anchor_broke_line {
                match after_dash_depth {
                    Some(depth) => checked_depth_add(depth, 1)?,
                    None => self.state.depth,
                }
            } else {
                self.state.depth
            };
            self.write_indent(depth)?;
        }
        self.write_key_scalar(variant)?;
        // Write ':' without a space and defer spacing/newline to the value serializer.
        self.out.write_str(":")?;
        self.state.pending_layout.pending_space_after_colon = true;
        self.state.at_line_start = false;
        // Do not let SeqSer's "inline first key after dash" hint leak into the variant's inner value.
        // Without this, a struct/map value can start as `Variant: a: 1`.
        self.state.pending_layout.pending_inline_map = false;
        // If this variant is inside a block sequence element (`- Variant:`), ensure the nested
        // value indents under the variant label rather than aligning with the list indentation.
        // SeqSer stores the dash's indentation depth in `after_dash_depth`.
        if let Some(d) = self.state.after_dash_depth.take() {
            let nested_depth = checked_depth_add(d, 1)?;
            let prev_map_depth = self.state.current_map_depth.replace(nested_depth);
            let res = value.serialize(&mut *self);
            self.state.current_map_depth = prev_map_depth;
            res
        } else {
            value.serialize(&mut *self)
        }
    }

    // -------- Collections --------

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        let flow = self.take_flow_for_seq();
        if flow {
            // Ensure a space after a preceding colon when this sequence is a mapping value.
            self.write_space_if_pending()?;
            self.write_scalar_prefix_if_anchor()?;
            if self.state.at_line_start {
                self.write_indent(self.state.depth)?;
            }
            self.out.write_str("[")?;
            self.state.at_line_start = false;
            let depth_next = self.state.depth; // inline
            Ok(SeqSer {
                ser: self,
                depth: depth_next,
                flow: true,
                first: true,
                force_empty_marker: false,
            })
        } else {
            // Block sequence. Decide indentation based on whether this is after a map key or after a list dash.
            let was_inline_value = !self.state.at_line_start;

            // If we are a value following a block sibling, force a newline now.
            // However, if a complex-node anchor is pending, we must keep `key: &aN` inline;
            // `write_anchor_for_complex_node` will handle emitting the anchor and newline.
            if self.state.pending_layout.pending_space_after_colon
                && self.state.last_value_was_block
                && !self.has_pending_node_properties()
            {
                self.state.pending_layout.pending_space_after_colon = false;
                if !self.state.at_line_start {
                    self.newline()?;
                }
                // Consume the sibling-block marker; it should not affect nested nodes.
                self.state.last_value_was_block = false;
            }

            // For block sequences nested under another dash, keep the first inner dash inline.
            // Style expectations in tests prefer the compact form:
            // - - 1
            // instead of:
            // -
            //   - 1
            let inline_first = (!self.state.at_line_start)
                && self.state.after_dash_depth.is_some()
                && !self.state.pending_layout.pending_space_after_colon;
            // `inline_first` assumes we stay mid-line, but a pending anchor writes `&aN\n` first.
            let anchor_broke_line = self.has_pending_node_properties();
            self.write_anchor_for_complex_node()?;
            if inline_first {
                if anchor_broke_line {
                    // Inlining now would drop the nested dashes to column 0, past the anchor.
                    self.state.pending_layout.pending_inline_map = false;
                } else {
                    // Collapsing onto the parent dash yields the preferred `- - 1` shape.
                    self.state.at_line_start = false;
                }
            } else if was_inline_value {
                // Mid-line start. If we are here due to a map value (after ':'), defer the newline
                // decision until the first element is emitted so that empty sequences can stay inline
                // as `key: []`. If we are here due to a list dash, keep inline.
                // Intentionally do not clear `pending_space_after_colon` and do not newline here.
            }
            // Indentation policy mirrors serialize_map:
            // - After a list dash inline_first: base is dash depth; indent one level deeper.
            // - As a value after a map key: base is current_map_depth (if set), indent one level deeper.
            // - Otherwise (top-level or already at line start): base is current depth.
            let base = if inline_first {
                self.state.after_dash_depth.unwrap_or(self.state.depth)
            } else if was_inline_value && self.state.current_map_depth.is_some() {
                self.state.current_map_depth.unwrap_or(self.state.depth)
            } else {
                self.state.depth
            };
            // For sequences used as a mapping value, indent them one level deeper so the dash is
            // nested under the parent key (consistent with serde_yaml's formatting). Keep block
            // sequences inline only when they immediately follow another dash.
            let depth_next = if inline_first {
                checked_depth_add(base, 1)?
            } else if was_inline_value {
                if self.settings.compact_list_indent
                    && self.state.current_map_depth.is_some()
                    && !anchor_broke_line
                {
                    base
                } else {
                    checked_depth_add(base, 1)?
                }
            } else {
                base
            };
            // Starting a complex (block) sequence: drop any staged inline comment.
            self.state.pending_inline_comment = None;
            Ok(SeqSer {
                ser: self,
                depth: depth_next,
                flow: false,
                first: true,
                force_empty_marker: anchor_broke_line,
            })
        }
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        if name == NAME_TUPLE_ANCHOR {
            Ok(TupleSer::anchor_strong(self))
        } else if name == NAME_TUPLE_WEAK {
            Ok(TupleSer::anchor_weak(self))
        } else if name == NAME_TUPLE_COMMENTED {
            Ok(TupleSer::commented(self))
        } else if name == NAME_TUPLE_TAGGED {
            Ok(TupleSer::tagged(self))
        } else {
            // Normal tuple-struct: emit as a block sequence.
            Ok(TupleSer::sequence(self.serialize_seq(Some(len))?))
        }
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        if self.pending_tag_selects_variant(variant) {
            return self.serialize_seq(Some(len));
        }

        let was_inline_value = self.state.pending_layout.pending_space_after_colon;
        let anchor_broke_line = self.has_pending_node_properties();
        let after_dash_depth = self.state.after_dash_depth;
        self.write_anchor_for_complex_node()?;

        // If we are the value of a mapping key, YAML forbids keeping a nested mapping
        // on the same line (e.g., "key: Variant:"). Move the variant mapping to the next line
        // indented under the parent mapping's base depth.
        if was_inline_value {
            self.state.pending_layout.pending_space_after_colon = false;
            if !self.state.at_line_start {
                self.newline()?;
            }
            let base =
                checked_depth_add(self.state.current_map_depth.unwrap_or(self.state.depth), 1)?;
            self.write_indent(base)?;
            self.write_key_scalar(variant)?;
            self.out.write_str(":\n")?;
            self.state.at_line_start = true;
            self.state.pending_layout.pending_inline_map = false;
            let depth_next = checked_depth_add(base, 1)?;
            return Ok(SeqSer {
                ser: self,
                depth: depth_next,
                flow: false,
                first: true,
                force_empty_marker: false,
            });
        }
        // Otherwise (top-level or sequence context).
        if self.state.at_line_start {
            let depth = if anchor_broke_line {
                match after_dash_depth {
                    Some(depth) => checked_depth_add(depth, 1)?,
                    None => self.state.depth,
                }
            } else {
                self.state.depth
            };
            self.write_indent(depth)?;
        }
        self.write_key_scalar(variant)?;
        self.out.write_str(":\n")?;
        self.state.at_line_start = true;
        let mut depth_next = checked_depth_add(self.state.depth, 1)?;
        if let Some(d) = self.state.after_dash_depth.take() {
            depth_next = checked_depth_add(d, 2)?;
            self.state.pending_layout.pending_inline_map = false;
        }
        Ok(SeqSer {
            ser: self,
            depth: depth_next,
            flow: false,
            first: true,
            force_empty_marker: false,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let flow = self.take_flow_for_map();
        if flow {
            // Ensure a space after a preceding colon when this mapping is a value.
            self.write_space_if_pending()?;
            self.write_scalar_prefix_if_anchor()?;
            if self.state.at_line_start {
                self.write_indent(self.state.depth)?;
            }
            self.out.write_str("{")?;
            self.state.at_line_start = false;
            let depth_next = self.state.depth;
            Ok(MapSer::flow(self, depth_next))
        } else {
            let node_properties_broke_line = self.has_pending_node_properties();
            let inline_first = self.state.pending_layout.pending_inline_map;
            // Starting a complex (block) map: drop any staged inline comment.
            self.state.pending_inline_comment = None;
            // We only consider "value position" when immediately after a mapping colon.
            let was_inline_value = self.state.pending_layout.pending_space_after_colon;
            let mut forced_newline = false;

            // If we are a value following a block sibling, force a newline now.
            // However, if a complex-node anchor is pending, we must keep `key: &aN` inline;
            // `write_anchor_for_complex_node` will handle emitting the anchor and newline.
            if was_inline_value
                && self.state.last_value_was_block
                && !self.has_pending_node_properties()
            {
                self.state.pending_layout.pending_space_after_colon = false;
                if !self.state.at_line_start {
                    self.newline()?;
                }
                forced_newline = true;
                // Consume the sibling-block marker; it should not affect nested nodes.
                self.state.last_value_was_block = false;
            }

            self.write_anchor_for_complex_node()?;
            if inline_first {
                // Suppress newline after a list dash for inline map first key.
                self.state.pending_layout.pending_inline_map = false;
                // Mark that this sequence element is a mapping printed inline after a dash.
                self.state.pending_layout.inline_map_after_dash = true;
            } else if was_inline_value {
                // Map used as a value after "key: ". If emitting braces for empty maps,
                // keep this mapping on the same line so that an empty map renders as "{}".
                //
                // IMPORTANT: if the map is known to be non-empty (len > 0), we must NOT keep it
                // inline (otherwise we can end up emitting the first entry as `key: a: 1`).
                // When len is unknown, we keep the legacy behavior and let MapSer decide once the
                // first key arrives.
                let known_empty = matches!(len, Some(0));
                let known_non_empty = matches!(len, Some(n) if n > 0);

                if !self.settings.empty_as_braces || known_non_empty {
                    // Move the mapping body to the next line.
                    // If an anchor was emitted, we are already at the start of a new line.
                    self.state.pending_layout.pending_space_after_colon = false;
                    if !self.state.at_line_start {
                        self.newline()?;
                    }
                } else if !known_empty {
                    // len is unknown: keep it inline for now (so empty maps can still render as
                    // `key: {}`), and let MapSer break the line when the first key arrives.
                }
            }
            // Indentation rules:
            // - Top-level (at line start, not after dash): use current depth.
            // - After dash inline first key or as a value: indent one level deeper for subsequent lines.
            // Use the current mapping's depth as base only when we are in a VALUE position.
            // For complex KEYS (non-scalar), keep using the current serializer depth so that
            // subsequent key lines indent relative to the "? " line, not the parent map's base.
            let base = if inline_first {
                self.state.after_dash_depth.unwrap_or(self.state.depth)
            } else if was_inline_value && self.state.current_map_depth.is_some() {
                self.state.current_map_depth.unwrap_or(self.state.depth)
            } else {
                self.state.depth
            };
            let depth_next = if inline_first || was_inline_value {
                checked_depth_add(base, 1)?
            } else {
                base
            };
            let inline_value_start_flag = was_inline_value
                && self.settings.empty_as_braces
                && len.is_none()
                && !inline_first
                && !forced_newline;
            Ok(MapSer::block(
                self,
                depth_next,
                inline_first,
                inline_value_start_flag,
                node_properties_broke_line,
            ))
        }
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        if self.pending_tag_selects_variant(variant) {
            return Ok(StructVariantSer::payload(self.serialize_map(Some(len))?));
        }

        let was_inline_value = self.state.pending_layout.pending_space_after_colon;
        let anchor_broke_line = self.has_pending_node_properties();
        let after_dash_depth = self.state.after_dash_depth;
        self.write_anchor_for_complex_node()?;

        // If we are the value of a mapping key, YAML forbids keeping a nested mapping
        // on the same line (e.g., "key: Variant:"). Move the variant mapping to the next line
        // indented under the parent mapping's base depth.
        if was_inline_value {
            // Value position after a map key: start the variant mapping on the next line.
            self.state.pending_layout.pending_space_after_colon = false;
            if !self.state.at_line_start {
                self.newline()?;
            }
            // Indent the variant name one level under the parent mapping.
            let base =
                checked_depth_add(self.state.current_map_depth.unwrap_or(self.state.depth), 1)?;
            self.write_indent(base)?;
            self.write_key_scalar(variant)?;
            self.out.write_str(":\n")?;
            self.state.at_line_start = true;
            // A complex key stages an inline hint for its value; clear it before the fields.
            self.state.pending_layout.pending_inline_map = false;
            // Fields indent one more level under the variant label.
            let depth_next = checked_depth_add(base, 1)?;
            return Ok(StructVariantSer::external(self, depth_next));
        }
        // Otherwise (top-level or sequence context), emit the variant name at current depth.
        if self.state.at_line_start {
            let depth = if anchor_broke_line {
                match after_dash_depth {
                    Some(depth) => checked_depth_add(depth, 1)?,
                    None => self.state.depth,
                }
            } else {
                self.state.depth
            };
            self.write_indent(depth)?;
        }
        self.write_key_scalar(variant)?;
        self.out.write_str(":\n")?;
        self.state.at_line_start = true;
        // Default indentation for fields under a plain variant line.
        let mut depth_next = checked_depth_add(self.state.depth, 1)?;
        // If this variant follows a list dash, indent two levels under the dash (one for the element, one for the mapping).
        if let Some(d) = self.state.after_dash_depth.take() {
            depth_next = checked_depth_add(d, 2)?;
            self.state.pending_layout.pending_inline_map = false;
        }
        Ok(StructVariantSer::external(self, depth_next))
    }
}

#[cfg(test)]
mod tests {
    use super::{YamlSerializer, absolute_uri_is_valid};

    #[test]
    fn absolute_uri_validation_covers_global_tag_syntax() {
        for valid in [
            "tag:yaml.org,2002:str",
            "urn:isbn:9780141036144",
            "https://example.com/path?query=yes#section",
            "https://example.com",
            "git+ssh://user@example.com:22/repository",
            "custom:",
            "http://[2001:db8::1]:8080/path",
            "http://[v1.alpha:beta]/path",
            "tag:escaped%20text",
        ] {
            assert!(absolute_uri_is_valid(valid), "expected valid URI: {valid}");
        }

        for invalid in [
            "",
            "$:?",
            "relative/path",
            "1scheme:value",
            "bad scheme:value",
            "http://[not-ip]/",
            "http://[v1]/",
            "http://[2001:db8::1/",
            "http://example.com:port/",
            "http://host:name:80/",
            "http://bad[info@example.com/",
            "http://first@second@third/",
            "http://example.com/path[0]",
            "tag:value?bad[query]",
            "tag:value#bad[fragment]",
            "tag:bad%escape",
        ] {
            assert!(
                !absolute_uri_is_valid(invalid),
                "expected invalid URI: {invalid}"
            );
        }
    }

    #[test]
    fn write_indent_rejects_indentation_overflow() {
        let mut out = String::new();
        let mut serializer = YamlSerializer::new(&mut out);
        let err = serializer.write_indent(usize::MAX).unwrap_err();
        assert_eq!(err.to_string(), "serializer indentation exceeds usize");
        assert!(out.is_empty());
    }
}
