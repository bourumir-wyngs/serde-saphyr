#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Protocol {
    Vast,
    OpenRtb,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Config {
    protocol: Protocol,
    after: u32,
}

#[rstest]
#[case::empty("")]
#[case::tilde("~")]
#[case::null("null")]
#[case::titlecase_null("Null")]
#[case::uppercase_null("NULL")]
fn tagged_unit_variant_accepts_null(
    #[case] payload: &str,
    #[values("vast", "open_rtb")] variant: &str,
) {
    let yaml = format!("protocol: !{variant} {payload}\nafter: 42\n");
    let expected = Config {
        protocol: match variant {
            "vast" => Protocol::Vast,
            _ => Protocol::OpenRtb,
        },
        after: 42,
    };
    let results: [Result<Config, serde_saphyr::Error>; 3] = [
        serde_saphyr::from_str(&yaml),
        serde_saphyr::from_slice(yaml.as_bytes()),
        serde_saphyr::from_reader(yaml.as_bytes()),
    ];

    for result in results {
        assert_eq!(
            result.expect("a null unit payload should deserialize"),
            expected
        );
    }
}

#[rstest]
#[case::vast("vast")]
#[case::open_rtb("open_rtb")]
#[case::boolean("false")]
#[case::integer("42")]
#[case::float("1.5")]
#[case::quoted_empty("\"\"")]
#[case::quoted_null("\"null\"")]
#[case::quoted_tilde("'~'")]
#[case::empty_mapping("{}")]
#[case::mapping("{a: 1}")]
#[case::empty_sequence("[]")]
#[case::sequence("[1, 2]")]
#[case::block_mapping("\n  a: 1")]
#[case::block_sequence("\n  - 1\n  - 2")]
fn tagged_unit_variant_rejects_non_null(
    #[case] payload: &str,
    #[values("vast", "open_rtb")] variant: &str,
) {
    let yaml = format!("protocol: !{variant} {payload}\nafter: 42\n");
    let results: [Result<Config, serde_saphyr::Error>; 3] = [
        serde_saphyr::from_str(&yaml),
        serde_saphyr::from_slice(yaml.as_bytes()),
        serde_saphyr::from_reader(yaml.as_bytes()),
    ];

    for result in results {
        let error = result.expect_err("a unit variant must reject non-null payloads");
        assert!(matches!(
            error.without_snippet(),
            serde_saphyr::Error::UnexpectedValueForUnit { .. }
        ));
    }
}
