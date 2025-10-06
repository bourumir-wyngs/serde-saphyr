//! Serializer options for YAML emission.
//!
//! Controls indentation and optional anchor name generation for the serializer.
//!
//! Example: use 4-space indentation and a custom anchor naming scheme.
//!
//! ```rust
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Item { a: i32, b: bool }
//!
//! let mut buf = String::new();
//! let opts = serde_saphyr::SerializerOptions {
//!     indent_step: 4,
//!     anchor_generator: Some(|id| format!("id{}/", id)),
//! };
//! serde_saphyr::to_writer_with_options(&mut buf, &Item { a: 1, b: true }, opts).unwrap();
//! assert!(buf.contains("a: 1"));
//! ```
#[derive(Clone, Copy)]
pub struct SerializerOptions {
    /// Number of spaces to indent per nesting level when emitting block-style collections.
    pub indent_step: usize,
    /// Optional custom anchor-name generator.
    ///
    /// Receives a monotonically increasing `usize` id (starting at 1) and returns the
    /// anchor name to emit. If `None`, the built-in generator yields names like `a1`, `a2`, ...
    pub anchor_generator: Option<fn(usize) -> String>,
}

impl Default for SerializerOptions {
    fn default() -> Self { Self { indent_step: 2, anchor_generator: None } }
}
