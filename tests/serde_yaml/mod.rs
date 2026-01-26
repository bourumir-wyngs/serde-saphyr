use serde_saphyr::{Budget, Options};

mod test_anchor_only;
mod test_binary;
mod test_block_scalars;
mod test_bytes;
mod test_composite_keys;
mod test_custom_date_format;
mod test_de;
mod test_end_marker;
mod test_enum_alias_nested;
mod test_enum_external;
mod test_enum_repetition_limit;
mod test_flow_map;
mod test_flow_seq;
mod test_from_str_value;
mod test_historical_failures;
mod test_io_helpers;
mod test_json;
mod test_limit_deserializer_options;
mod test_limit_recursion;
mod test_limit_repetition;
mod test_merge_keys_serde;
mod test_multi_helpers;
mod test_no_panic;
mod test_numeric_enums;
mod test_rc_arc_cow_etc;
mod test_readme_examples;
mod test_serde;
mod test_stream_deserializer;
mod test_string_quoting;
mod test_writer_reader;
// This test takes too long in "test" configuration. If must only run in "release"
// and the fuzz folder is currently too many large files to be practical to commit.
//mod test_repro_fuzz_targets;

pub fn adapt_to_miri() -> Options {
    // Tighten limits for miri that otherwise takes very long
    if cfg!(miri) {
        serde_saphyr::options! {
            budget: Some(Budget {
                max_nodes: 250,
                ..Budget::default()
            }),
        }
    } else {
        Options::default()
    }
}
