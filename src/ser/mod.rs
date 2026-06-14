//! Single-pass YAML serializer with optional anchors for Rc/Arc/Weak,
//! order preservation (uses the iterator order of your types), simple
//! style controls (block strings & flow containers), and special
//! float handling for NaN/±Inf. No intermediate YAML DOM is built.
//!
//! Usage example:
//!
//! ```
//! use serde::Serialize;
//! use std::rc::Rc;
//! use serde_saphyr::{to_string, RcAnchor, LitStr, FlowSeq};
//!
//! #[derive(Serialize)]
//! struct Cfg {
//!     name: String,
//!     ports: FlowSeq<Vec<u16>>,   // render `[8080, 8081]`
//!     note: LitStr<'static>,      // render as `|` block
//!     data: RcAnchor<Vec<i32>>,   // first sight => &a1
//!     alias: RcAnchor<Vec<i32>>,  // later sight => *a1
//! }
//!
//! fn main() {
//!     let shared = Rc::new(vec![1,2,3]);
//!     let cfg = Cfg {
//!         name: "demo".into(),
//!         ports: FlowSeq(vec![8080, 8081]),
//!         note: LitStr("line 1\nline 2"),
//!         data: RcAnchor(shared.clone()),
//!         alias: RcAnchor(shared),
//!     };
//!     println!("{}", to_string(&cfg).unwrap());
//! }
//! ```

pub(crate) mod api;
pub mod error;
pub mod options;
pub(crate) mod quoting;
mod serializer;
mod wrapper_impls;
mod wrapping;
mod zmij_format;

pub use self::error::Error;
pub use self::serializer::{
    MapSer, SeqSer, StructVariantSer, TupleSer, YamlSerializer,
};

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

pub use crate::long_strings::{FoldStr, FoldString, LitStr, LitString};

// Flow hints and block-string hints: we use newtype-struct names.
const NAME_TUPLE_ANCHOR: &str = "__yaml_anchor";
const NAME_TUPLE_WEAK: &str = "__yaml_weak_anchor";
const NAME_FLOW_SEQ: &str = "__yaml_flow_seq";
const NAME_FLOW_MAP: &str = "__yaml_flow_map";
const NAME_TUPLE_COMMENTED: &str = "__yaml_commented";
const NAME_SPACE_AFTER: &str = "__yaml_space_after";
