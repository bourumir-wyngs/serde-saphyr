/// Requirements for indentation validation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub fn is_valid(&mut self, n: usize) -> bool {
        match self {
            RequireIndent::Unchecked => true,
            RequireIndent::Divisible(d) => n % *d == 0,
            RequireIndent::Even => n % 2 == 0,
            RequireIndent::Uniform(remembered) => {
                if n == 0 {
                    return true;
                }
                match *remembered {
                    None => {
                        *remembered = Some(n);
                        true
                    }
                    Some(expected) => n % expected == 0,
                }
            }
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
        assert!(r.is_valid(0));
        assert!(r.is_valid(1));
        assert!(r.is_valid(7));
        assert!(r.is_valid(100));
    }

    // --- is_valid: Divisible ---

    #[test]
    fn divisible_by_4() {
        let mut r = RequireIndent::Divisible(4);
        assert!(r.is_valid(0));
        assert!(r.is_valid(4));
        assert!(r.is_valid(8));
        assert!(!r.is_valid(1));
        assert!(!r.is_valid(3));
        assert!(!r.is_valid(5));
    }

    #[test]
    fn divisible_by_1() {
        let mut r = RequireIndent::Divisible(1);
        assert!(r.is_valid(0));
        assert!(r.is_valid(1));
        assert!(r.is_valid(999));
    }

    // --- is_valid: Even ---

    #[test]
    fn even_accepts_even_numbers() {
        let mut r = RequireIndent::Even;
        assert!(r.is_valid(0));
        assert!(r.is_valid(2));
        assert!(r.is_valid(4));
        assert!(r.is_valid(100));
    }

    #[test]
    fn even_rejects_odd_numbers() {
        let mut r = RequireIndent::Even;
        assert!(!r.is_valid(1));
        assert!(!r.is_valid(3));
        assert!(!r.is_valid(99));
    }

    // --- is_valid: Uniform ---

    #[test]
    fn uniform_remembers_first_nonzero() {
        let mut r = RequireIndent::Uniform(None);
        assert!(r.is_valid(0)); // zero always passes, doesn't set
        assert_eq!(r, RequireIndent::Uniform(None));
        assert!(r.is_valid(4)); // sets remembered to 4
        assert_eq!(r, RequireIndent::Uniform(Some(4)));
    }

    #[test]
    fn uniform_accepts_multiples_of_remembered() {
        let mut r = RequireIndent::Uniform(Some(2));
        assert!(r.is_valid(0));
        assert!(r.is_valid(2));
        assert!(r.is_valid(4));
        assert!(r.is_valid(6));
    }

    #[test]
    fn uniform_rejects_non_multiples() {
        let mut r = RequireIndent::Uniform(Some(4));
        assert!(!r.is_valid(3));
        assert!(!r.is_valid(5));
        assert!(!r.is_valid(7));
    }

    #[test]
    fn uniform_zero_always_valid_even_after_set() {
        let mut r = RequireIndent::Uniform(Some(3));
        assert!(r.is_valid(0));
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
