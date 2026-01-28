//! Tests confirming that `deserialize_string` (used by `Cow` and others)
//! attempts to borrow strings from the input when possible.

#[cfg(test)]
mod tests {
    use serde::de::{self, Deserializer, Visitor};
    use serde::Deserialize;
    use std::borrow::Cow;
    use std::fmt;
    use serde_saphyr::from_str;

    /// A wrapper around `Cow<'a, str>` that implements `Deserialize` via `deserialize_string`.
    /// It allows verifying which visitor method was called.
    #[derive(Debug)]
    struct VerifiableCow<'a>(Cow<'a, str>);

    impl<'de: 'a, 'a> Deserialize<'de> for VerifiableCow<'a> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct CowVisitor;

            impl<'de> Visitor<'de> for CowVisitor {
                type Value = VerifiableCow<'de>;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a string")
                }

                fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    // Success! The deserializer offered a borrowed string.
                    Ok(VerifiableCow(Cow::Borrowed(v)))
                }

                fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    // Fallback to owned.
                    Ok(VerifiableCow(Cow::Owned(v)))
                }
                
                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                     Ok(VerifiableCow(Cow::Owned(v.to_owned())))
                }
            }

            // Crucially, we request `deserialize_string`. 
            // Previous to the fix, this would ALWAYS result in `visit_string`.
            deserializer.deserialize_string(CowVisitor)
        }
    }

    #[test]
    fn test_deserialize_string_borrows_simple_scalar() {
        let input = "hello_world";
        let cow: VerifiableCow = from_str(input).unwrap();

        if let Cow::Owned(_) = cow.0 {
            panic!("deserialize_string failed to borrow a simple scalar!");
        }
        assert_eq!(cow.0, "hello_world");
    }

    #[test]
    fn test_deserialize_string_borrows_quoted_scalar() {
        // Quoted strings can be borrowed if they have no escapes
        let input = "\"hello world\"";
        let cow: VerifiableCow = from_str(input).unwrap();

        if let Cow::Owned(_) = cow.0 {
            panic!("deserialize_string failed to borrow a simple quoted scalar!");
        }
        assert_eq!(cow.0, "hello world");
    }

    #[test]
    fn test_deserialize_string_falls_back_to_owned_for_escapes() {
        // Escapes require transformation, so must be owned
        let input = "\"hello\\nworld\"";
        let cow: VerifiableCow = from_str(input).unwrap();

        match cow.0 {
            Cow::Owned(_) => {},
            Cow::Borrowed(_) => panic!("Should not borrow string with escapes!"),
        }
        assert_eq!(cow.0, "hello\nworld");
    }
}
