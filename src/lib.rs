#![forbid(unsafe_code)]

#[cfg(not(any(feature = "serialize", feature = "deserialize")))]
compile_error!(
    "Invalid feature configuration: enable at least one of \
     \"serialize\" or \"deserialize\"."
);

#[cfg(all(feature = "deserialize", feature = "properties"))]
pub use self::de::PropertySyntax;
#[cfg(all(feature = "deserialize", any(feature = "garde", feature = "validator")))]
pub use self::de_error::{ValidationIssue, ValidationSource};
#[cfg(feature = "serialize")]
pub use self::ser::{
    Error as SerializeError, error as ser_error,
    options::{CommentPosition, SerializerOptions},
};
#[cfg(feature = "deserialize")]
pub use self::{
    de::{
        Budget, DuplicateKeyPolicy, Error, Error as DeserializeError, MergeKeyPolicy, Options,
        budget, localizer, options,
    },
    de_error::{
        CroppedRegion, MessageFormatter, RenderOptions, SnippetMode, TransformReason,
        UserMessageFormatter,
    },
    indentation::RequireIndent,
    input_source::{
        IncludeRequest, IncludeResolveError, IncludeResolver, InputSource, ResolveProblem,
        ResolvedInclude,
    },
    localizer::{
        DEFAULT_ENGLISH_LOCALIZER, DefaultEnglishLocalizer, ExternalMessage, ExternalMessageSource,
        Localizer,
    },
    message_formatters::{DefaultMessageFormatter, DeveloperMessageFormatter},
};
pub use anchors::{
    ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor, RcAnchor, RcRecursion, RcRecursive,
    RcWeakAnchor,
};
#[cfg(feature = "figment")]
pub use de::figment;
#[cfg(feature = "figment2")]
pub use de::figment2;
#[cfg(feature = "miette")]
pub use de::miette;
#[cfg(any(feature = "garde", feature = "validator"))]
pub use de::path_map;
#[cfg(feature = "properties")]
pub use de::properties;
#[cfg(feature = "robotics")]
pub use de::robotics;
#[cfg(all(feature = "deserialize", feature = "include_fs"))]
pub use de::safe_resolver::{SafeFileReadMode, SafeFileResolver, SymlinkPolicy};
#[cfg(feature = "deserialize")]
pub use granit_parser;
pub use location::{Location, Locations};
pub use long_strings::{FoldStr, FoldString, LitStr, LitString};
pub use span::Span;
pub use spanned::Spanned;
#[cfg(any(feature = "serialize", feature = "deserialize"))]
pub use wrappers::{
    Commented, DoubleQuoted, FlowMap, FlowSeq, NullableTilde, SingleQuoted, SpaceAfter,
};

#[cfg(all(feature = "deserialize", feature = "include"))]
pub(crate) use self::de::api::resolver_from_options;
#[cfg(feature = "deserialize")]
pub use self::de::api::{
    from_multiple, from_multiple_with_options, from_reader, from_reader_with_options, from_slice,
    from_slice_multiple, from_slice_multiple_with_options, from_slice_with_options, from_str,
    from_str_with_options, read, read_with_options,
};
#[cfg(feature = "serialize")]
pub use self::ser::api::{
    to_fmt_writer, to_fmt_writer_with_options, to_io_writer, to_io_writer_with_options, to_string,
    to_string_multiple, to_string_multiple_with_options, to_string_with_options,
};

#[cfg(feature = "deserialize")]
mod anchor_store;
mod anchors;
#[cfg(all(
    feature = "serialize",
    feature = "deserialize",
    feature = "include",
    feature = "include_fs"
))]
#[doc(hidden)]
pub mod cli;
#[cfg(feature = "deserialize")]
mod de;
mod location;
mod long_strings;
mod parse_scalars;
#[cfg(feature = "serialize")]
pub mod ser;
mod span;
mod spanned;
#[cfg(any(feature = "serialize", feature = "deserialize"))]
mod wrappers;

#[cfg(all(feature = "deserialize", feature = "include"))]
pub(crate) use de::include_stack;
#[cfg(any(feature = "garde", feature = "validator"))]
use de::lib_validate;
#[cfg(feature = "deserialize")]
pub(crate) use de::{
    buffered_input, error as de_error, include, indentation, input_source, live_events,
    message_formatters, properties_redaction, snippet as de_snippet, tags,
};

#[cfg(feature = "deserialize")]
pub use de::YamlDeserializer as Deserializer;
#[cfg(any(feature = "garde", feature = "validator"))]
pub use lib_validate::*;
#[cfg(feature = "serialize")]
pub use ser::YamlSerializer as Serializer;

#[cfg(feature = "deserialize")]
pub use de::{
    with_deserializer_from_reader, with_deserializer_from_reader_with_options,
    with_deserializer_from_slice, with_deserializer_from_slice_with_options,
    with_deserializer_from_str, with_deserializer_from_str_with_options,
};

mod macros;
