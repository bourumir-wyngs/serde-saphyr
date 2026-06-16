use serde_saphyr::Quoted;

#[cfg(feature = "serialize")]
mod serialize_tests {
    use serde::Serialize;
    use serde_saphyr::Quoted;

    #[derive(Serialize)]
    struct QuotedDoc {
        text: Quoted<&'static str>,
        escaped: Quoted<&'static str>,
        owned: Quoted<String>,
    }

    #[test]
    fn quoted_forces_double_quoted_scalar_output() {
        let value = QuotedDoc {
            text: Quoted("plain"),
            escaped: Quoted("line\nbreak"),
            owned: Quoted("owned".to_string()),
        };

        let yaml = serde_saphyr::to_string(&value).unwrap();

        assert_eq!(
            yaml,
            "text: \"plain\"\nescaped: \"line\\nbreak\"\nowned: \"owned\"\n"
        );
    }
}

#[cfg(all(feature = "serialize", feature = "deserialize"))]
mod round_trip_tests {
    use serde::{Deserialize, Serialize};
    use serde_saphyr::Quoted;

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct ShoppingList {
        product: String,
        pass: Quoted<String>,
    }

    #[test]
    fn quoted_preserves_trailing_spaces_in_milk_sample() {
        let yaml = "product: milk\npass: \"trailing spaces important   \"\n";

        let list: ShoppingList = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(list.product, "milk");
        assert_eq!(
            list.pass,
            Quoted("trailing spaces important   ".to_string())
        );
        assert_eq!(serde_saphyr::to_string(&list).unwrap(), yaml);
    }
}

#[test]
fn quoted_forces_double_quoted_top_level_scalar_output() {
    let yaml = serde_saphyr::to_string(&Quoted("plain")).unwrap();
    assert_eq!(yaml, "\"plain\"\n");
}
