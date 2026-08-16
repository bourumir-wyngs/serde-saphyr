// See https://github.com/dtolnay/serde-yaml/issues/414 - plain numbers should
// deserialize into String consistently, including through Serde's buffered paths.
use serde::Deserialize;
use std::collections::HashMap;

#[test]
fn test_plain_number_as_string_through_serde_buffering() {
    #[derive(Deserialize)]
    struct Data {
        env: HashMap<String, String>,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum InlineUntagged {
        Data { env: HashMap<String, String> },
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NewtypeUntagged {
        Data(Data),
    }

    #[derive(Deserialize)]
    struct Flattened {
        #[serde(flatten)]
        data: Data,
    }

    let yaml = "env:\n  foo: 123\n";
    let cases = [
        (
            "direct struct",
            serde_saphyr::from_str::<Data>(yaml).map(|data| data.env["foo"].clone()),
        ),
        (
            "inline untagged enum",
            serde_saphyr::from_str::<InlineUntagged>(yaml).map(|data| match data {
                InlineUntagged::Data { env } => env["foo"].clone(),
            }),
        ),
        (
            "newtype untagged enum",
            serde_saphyr::from_str::<NewtypeUntagged>(yaml).map(|data| match data {
                NewtypeUntagged::Data(data) => data.env["foo"].clone(),
            }),
        ),
        (
            "flattened struct",
            serde_saphyr::from_str::<Flattened>(yaml).map(|data| data.data.env["foo"].clone()),
        ),
    ];

    let failures: Vec<_> = cases
        .into_iter()
        .filter_map(|(case, result)| match result {
            Ok(value) if value == "123" => None,
            Ok(value) => Some(format!("{case}: expected \"123\", got {value:?}")),
            Err(error) => Some(format!("{case}: {error}")),
        })
        .collect();

    assert!(
        failures.is_empty(),
        "plain number did not deserialize consistently:\n{}",
        failures.join("\n")
    );
}
