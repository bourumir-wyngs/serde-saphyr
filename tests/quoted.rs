use serde_saphyr::{DoubleQuoted, SingleQuoted};

#[cfg(feature = "serialize")]
mod serialize_tests {
    use serde::Serialize;
    use serde_saphyr::{DoubleQuoted, SingleQuoted};

    #[derive(Serialize)]
    struct DoubleQuotedDoc {
        text: DoubleQuoted<&'static str>,
        escaped: DoubleQuoted<&'static str>,
        quote: DoubleQuoted<&'static str>,
        owned: DoubleQuoted<String>,
    }

    #[test]
    fn double_quoted_forces_double_quoted_scalar_output() {
        let value = DoubleQuotedDoc {
            text: DoubleQuoted("plain"),
            escaped: DoubleQuoted("line\nbreak"),
            quote: DoubleQuoted("say \"hi\""),
            owned: DoubleQuoted("owned".to_string()),
        };

        let yaml = serde_saphyr::to_string(&value).unwrap();

        assert_eq!(
            yaml,
            "text: \"plain\"\nescaped: \"line\\nbreak\"\nquote: \"say \\\"hi\\\"\"\nowned: \"owned\"\n"
        );
    }

    #[derive(Serialize)]
    struct SingleQuotedDoc {
        text: SingleQuoted<&'static str>,
        escaped: SingleQuoted<&'static str>,
        owned: SingleQuoted<String>,
    }

    #[test]
    fn single_quoted_forces_single_quoted_scalar_output() {
        let value = SingleQuotedDoc {
            text: SingleQuoted("plain"),
            escaped: SingleQuoted("can't"),
            owned: SingleQuoted("owned".to_string()),
        };

        let yaml = serde_saphyr::to_string(&value).unwrap();

        assert_eq!(yaml, "text: 'plain'\nescaped: 'can''t'\nowned: 'owned'\n");
    }
}

#[cfg(all(feature = "serialize", feature = "deserialize"))]
mod round_trip_tests {
    use serde::{Deserialize, Serialize};
    use serde_saphyr::{DoubleQuoted, SingleQuoted};

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct ShoppingList {
        product: String,
        pass: DoubleQuoted<String>,
    }

    #[test]
    fn double_quoted_preserves_trailing_spaces_in_milk_sample() {
        let yaml = "product: milk\npass: \"trailing spaces important   \"\n";

        let list: ShoppingList = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(list.product, "milk");
        assert_eq!(
            list.pass,
            DoubleQuoted("trailing spaces important   ".to_string())
        );
        assert_eq!(serde_saphyr::to_string(&list).unwrap(), yaml);
    }

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Password {
        pass: SingleQuoted<String>,
    }

    #[test]
    fn single_quoted_preserves_trailing_spaces_in_milk_sample() {
        let yaml = "pass: 'trailing spaces important   '\n";

        let password: Password = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(
            password.pass,
            SingleQuoted("trailing spaces important   ".to_string())
        );
        assert_eq!(serde_saphyr::to_string(&password).unwrap(), yaml);
    }
}

#[test]
fn double_quoted_forces_double_quoted_top_level_scalar_output() {
    let yaml = serde_saphyr::to_string(&DoubleQuoted("plain")).unwrap();
    assert_eq!(yaml, "\"plain\"\n");
}

#[test]
fn single_quoted_forces_single_quoted_top_level_scalar_output() {
    let yaml = serde_saphyr::to_string(&SingleQuoted("plain")).unwrap();
    assert_eq!(yaml, "'plain'\n");
}
