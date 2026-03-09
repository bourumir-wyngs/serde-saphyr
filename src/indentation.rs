/// Requirements for indentation validation during YAML deserialization.
///
/// When set via [`options!`](crate::options), the deserializer validates every
/// parser-reported indentation level against the chosen policy.
///
/// # Variants
///
/// | Variant | Accepts |
/// |---------|---------|
/// | [`Unchecked`](Self::Unchecked) | Any indentation (default) |
/// | [`Divisible(n)`](Self::Divisible) | Indentation divisible by `n` |
/// | [`Even`](Self::Even) | Even indentation (0, 2, 4, …) |
/// | [`Uniform`](Self::Uniform) | Consistent indentation throughout the document |
///
/// *Important:* if there are multiple documents in the YAML input, or added with !include,
/// indentation requirements apply to all of them. For Uniform, indentation is required to be
/// consistent across all included documents as well.
///
/// # Examples
///
/// ## Enforcing even indentation
///
/// ```rust
/// use serde_json::Value;
///
/// // This YAML uses 2-space indentation — accepted by `Even`.
/// let yaml = r#"
/// server:
///   host: localhost
///   port: 8080
/// "#;
/// let options = serde_saphyr::options! {
///     require_indent: serde_saphyr::RequireIndent::Even,
/// };
/// let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
/// assert!(result.is_ok());
/// ```
///
/// ```rust
/// use serde_json::Value;
///
/// // This YAML uses 3-space indentation — rejected by `Even`.
/// let yaml = r#"
/// server:
///    host: localhost
/// "#;
/// let options = serde_saphyr::options! {
///     require_indent: serde_saphyr::RequireIndent::Even,
/// };
/// let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
/// assert!(result.is_err());
/// ```
///
/// ## Enforcing indentation divisible by 4
///
/// ```rust
/// use serde_json::Value;
///
/// // 4-space indentation — accepted by `Divisible(4)`.
/// let yaml = r#"
/// database:
///     host: db.local
///     port: 5432
/// "#;
/// let options = serde_saphyr::options! {
///     require_indent: serde_saphyr::RequireIndent::Divisible(4),
/// };
/// assert!(serde_saphyr::from_str_with_options::<Value>(yaml, options).is_ok());
/// ```
///
/// ## Enforcing uniform indentation
///
/// ```rust
/// use serde_json::Value;
///
/// // Consistent 2-space indentation throughout.
/// let yaml = r#"
/// app:
///   name: demo
///   version: 1
/// "#;
/// let options = serde_saphyr::options! {
///     require_indent: serde_saphyr::RequireIndent::Uniform(None),
/// };
/// assert!(serde_saphyr::from_str_with_options::<Value>(yaml, options).is_ok());
/// ```
///
/// ## YAML sample that triggers an indentation error
///
/// ```yaml
/// # With RequireIndent::Even, this document would fail:
/// config:
///    debug: true    # 3-space indent — odd, rejected
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RequireIndent {
    /// No indentation checking is performed.
    Unchecked,
    /// Indentation must be divisible by `n`.
    Divisible(usize),
    /// Indentation must be even.
    Even,
    /// Indentation must be uniform throughout the document.
    Uniform(Option<usize>),
}

impl RequireIndent {
    /// Checks whether the given indentation `n` is valid with respect to this requirement.
    ///
    /// For [`Uniform`](RequireIndent::Uniform), the first non-zero indentation encountered is
    /// remembered, and subsequent calls compare against that stored value.
    ///
    /// Returns `Ok(())` if valid, or an [`Error::IndentationError`](crate::de_error::Error::IndentationError)
    /// with [`Location::UNKNOWN`](crate::Location) — the caller is expected to attach the
    /// correct location later via [`Error::with_location`](crate::de_error::Error::with_location).
    pub fn is_valid(&mut self, n: usize) -> Result<(), crate::de_error::Error> {
        let ok = match self {
            RequireIndent::Unchecked => true,
            RequireIndent::Divisible(d) => n.is_multiple_of(*d),
            RequireIndent::Even => n.is_multiple_of(2),
            RequireIndent::Uniform(remembered) => {
                if n == 0 {
                    return Ok(());
                }
                match *remembered {
                    None => {
                        *remembered = Some(n);
                        true
                    }
                    Some(expected) => n.is_multiple_of(expected),
                }
            }
        };
        if ok {
            Ok(())
        } else {
            Err(crate::de_error::Error::IndentationError {
                required: *self,
                actual: n,
                location: crate::location::Location::UNKNOWN,
            })
        }
    }
}

impl std::fmt::Display for RequireIndent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequireIndent::Unchecked => write!(f, "unchecked"),
            RequireIndent::Divisible(n) => write!(f, "divisible by {n}"),
            RequireIndent::Even => write!(f, "even"),
            RequireIndent::Uniform(Some(n)) => write!(f, "uniform ({n} spaces)"),
            RequireIndent::Uniform(None) => write!(f, "uniform"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_valid: Unchecked ---

    #[test]
    fn unchecked_always_valid() {
        let mut r = RequireIndent::Unchecked;
        r.is_valid(0).unwrap();
        r.is_valid(1).unwrap();
        r.is_valid(7).unwrap();
        r.is_valid(100).unwrap();
    }

    // --- is_valid: Divisible ---

    #[test]
    fn divisible_by_4() {
        let mut r = RequireIndent::Divisible(4);
        r.is_valid(0).unwrap();
        r.is_valid(4).unwrap();
        r.is_valid(8).unwrap();
        r.is_valid(1).unwrap_err();
        r.is_valid(3).unwrap_err();
        r.is_valid(5).unwrap_err();
    }

    #[test]
    fn divisible_by_1() {
        let mut r = RequireIndent::Divisible(1);
        r.is_valid(0).unwrap();
        r.is_valid(1).unwrap();
        r.is_valid(999).unwrap();
    }

    // --- is_valid: Even ---

    #[test]
    fn even_accepts_even_numbers() {
        let mut r = RequireIndent::Even;
        r.is_valid(0).unwrap();
        r.is_valid(2).unwrap();
        r.is_valid(4).unwrap();
        r.is_valid(100).unwrap();
    }

    #[test]
    fn even_rejects_odd_numbers() {
        let mut r = RequireIndent::Even;
        r.is_valid(1).unwrap_err();
        r.is_valid(3).unwrap_err();
        r.is_valid(99).unwrap_err();
    }

    // --- is_valid: Uniform ---

    #[test]
    fn uniform_remembers_first_nonzero() {
        let mut r = RequireIndent::Uniform(None);
        r.is_valid(0).unwrap(); // zero always passes, doesn't set
        assert_eq!(r, RequireIndent::Uniform(None));
        r.is_valid(4).unwrap(); // sets remembered to 4
        assert_eq!(r, RequireIndent::Uniform(Some(4)));
    }

    #[test]
    fn uniform_accepts_multiples_of_remembered() {
        let mut r = RequireIndent::Uniform(Some(2));
        r.is_valid(0).unwrap();
        r.is_valid(2).unwrap();
        r.is_valid(4).unwrap();
        r.is_valid(6).unwrap();
    }

    #[test]
    fn uniform_rejects_non_multiples() {
        let mut r = RequireIndent::Uniform(Some(4));
        r.is_valid(3).unwrap_err();
        r.is_valid(5).unwrap_err();
        r.is_valid(7).unwrap_err();
    }

    #[test]
    fn uniform_zero_always_valid_even_after_set() {
        let mut r = RequireIndent::Uniform(Some(3));
        r.is_valid(0).unwrap();
    }

    // --- Display ---

    #[test]
    fn display_unchecked() {
        assert_eq!(RequireIndent::Unchecked.to_string(), "unchecked");
    }

    #[test]
    fn display_divisible() {
        assert_eq!(RequireIndent::Divisible(4).to_string(), "divisible by 4");
    }

    #[test]
    fn display_even() {
        assert_eq!(RequireIndent::Even.to_string(), "even");
    }

    #[test]
    fn display_uniform_none() {
        assert_eq!(RequireIndent::Uniform(None).to_string(), "uniform");
    }

    #[test]
    fn display_uniform_some() {
        assert_eq!(
            RequireIndent::Uniform(Some(2)).to_string(),
            "uniform (2 spaces)"
        );
    }
}
