//! Unicode-specific edge-case tests for zero-copy deserialization.

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Data<'a> {
        text: &'a str,
    }

    #[test]
    fn borrow_unicode_ascii_mix() {
        let yaml = "text: \u{1F980} and friends\n";
        let result: Data = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.text, "\u{1F980} and friends");
    }

    #[test]
    fn borrow_unicode_at_boundaries() {
        let yaml = "text: \"\u{1F980}\"\n"; // Double quoted, now should borrow
        let result: Data = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.text, "\u{1F980}");
        
        let yaml_plain = "text: \u{1F980}\n";
        let result_plain: Data = serde_saphyr::from_str(yaml_plain).unwrap();
        assert_eq!(result_plain.text, "\u{1F980}");
    }

    #[test]
    fn borrow_multiple_unicode() {
        let yaml = "text: \u{1F980}\u{1F525}\u{1F680}\n";
        let result: Data = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.text, "\u{1F980}\u{1F525}\u{1F680}");
    }

    #[test]
    fn borrow_unicode_with_spaces() {
        let yaml = "text:  \u{1F980}  \n";
        let result: Data = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.text, "\u{1F980}");
    }
}
