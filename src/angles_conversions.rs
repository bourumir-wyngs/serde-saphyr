/// Parse and evaluate a YAML 1.2 float scalar with robotics-style angle support.
///
/// This function accepts plain numbers, expressions, and unit functions,
/// evaluates them, and returns the numeric value (converted to radians when
/// applicable). It uses a small recursive-descent evaluator supporting `+ - * /`
/// with parentheses and constants.
///
/// # Supported features
/// - YAML 1.2 float forms: `0.15`, `1e-3`, `.inf`, `.nan`, etc. (case-insensitive)
/// - Constants: `pi`, `tau`, `inf`, `nan`
/// - Expressions: `2*pi`, `1 + 2*(3 - 4/5)`, `pi/2`
/// - Unit functions:
///     - `deg(<expr>)` — interpret as degrees, convert to radians
///     - `rad(<expr>)` — interpret as radians (no conversion)
///
/// # Tag interaction
/// - If no `deg`/`rad` is used, `SfTag::Degrees` converts to radians.
/// - `SfTag::Radians` leaves values as-is.
/// - Unit functions override tag-based conversion.
///
/// # Errors
/// Returns a descriptive [`Error`] with [`Location`] for malformed syntax,
/// unbalanced parentheses, or unknown identifiers.
///
/// # Examples
/// ```
/// use crate::{SfTag, Location};
/// let v: f64 = parse_yaml12_float_angle_converting("deg(180)", Location::UNKNOWN, SfTag::Radians).unwrap();
/// assert!((v - std::f64::consts::PI).abs() < 1e-12);
/// ```
use core::f64::consts::PI;
use core::str::FromStr;

// Adjust imports to your crate layout:
use crate::tags::SfTag;
use crate::{Error, Location};

/// Minimal conversion helper so callers can choose `T = f64` or `T = f32`.
pub trait FromF64 {
    /// Construct from an f64 (lossy for f32).
    fn from_f64(v: f64) -> Self;
}
impl FromF64 for f64 {
    #[inline]
    fn from_f64(v: f64) -> Self {
        v
    }
}
impl FromF64 for f32 {
    #[inline]
    fn from_f64(v: f64) -> Self {
        v as f32
    }
}

// -----------------------------------------------------------------------------
// parse_yaml12_float_angle_converting
//
// This function parses and evaluates numeric expressions with optional
// angle-unit handling, returning a numeric value converted to radians when
// applicable.
//
// Core behavior:
// • Implements a small recursive-descent evaluator supporting +, -, *, /,
//   parentheses, constants (pi, tau, inf, nan), and functions deg(...)/rad(...).
// • Accepts YAML 1.2–style float syntax: decimal, scientific, and special
//   forms (.inf, .nan, inf, nan) in any case.
// • Evaluates expressions directly (no AST), using double precision (f64)
//   internally, and converts the final result to the generic type `T` via
//   the `FromF64` trait.
// • Recognizes unit functions:
//       deg(expr) – interprets argument in degrees, converts to radians.
//       rad(expr) – interprets argument already in radians.
//   These override any tag-based conversion.
// • If no explicit unit function is used, the YAML tag determines whether
//   to convert degrees→radians (`SfTag::Degrees`) or leave as-is
//   (`SfTag::Radians`); other tags are ignored.
// • Enforces full expression parsing (no trailing garbage) and produces
//   descriptive `Error`s with source `Location` for malformed syntax.
//
// Grammar summary:
//     expr   := term (('+' | '-') term)*
//     term   := unary (('*' | '/') unary)*
//     unary  := ('+' | '-')* primary
//     primary:= NUMBER | CONST | '(' expr ')' | FUNC '(' expr ')'
//
// Examples:
//     "2*pi"           → 6.283185307179586
//     "deg(180)"       → 3.141592653589793
//     "1 + 2*(3 - 4/5)"→ 5.4
//
// Intended for robotics YAML parsing (ROS/URDF/MoveIt! style) where angles
// may be expressed as numeric expressions or in degrees/radians.
// -----------------------------------------------------------------------------
pub(crate) fn parse_yaml12_float_angle_converting<T>(
    s: &str,
    location: Location,
    tag: SfTag,
) -> Result<T, Error>
where
    T: FromF64,
{
    let mut p = Parser::new(s, location);
    p.skip_ws();
    let (mut value, used_unit) = p.expr()?; // parse whole expression
    p.skip_ws();
    if !p.eof() {
        return Err(p.err("unexpected trailing characters in scalar"));
    }

    // Apply tag-based conversion only if no explicit unit function was used.
    if !used_unit {
        match tag {
            SfTag::Degrees => value *= PI / 180.0,
            SfTag::Radians => { /* already radians */ }
            _ => { /* ignore other tags for floats */ }
        }
    }

    Ok(T::from_f64(value))
}

/* ----------------------------- Parser impl ------------------------------ */

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
    loc: Location,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str, loc: Location) -> Self {
        Self {
            s,
            b: s.as_bytes(),
            i: 0,
            loc: loc,
        }
    }
    #[inline]
    fn eof(&self) -> bool {
        self.i >= self.b.len()
    }
    #[inline]
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    #[inline]
    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.i += 1;
        Some(c)
    }
    #[inline]
    fn is_ws(c: u8) -> bool {
        matches!(c, b' ' | b'\t' | b'\n' | b'\r')
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if !Self::is_ws(c) {
                break;
            }
            self.i += 1;
        }
    }
    fn err(&self, msg: &str) -> Error {
        // Adjust to your error constructor if needed.
        Error::HookError {
            msg: msg.to_string(),
            location: self.loc,
        }
    }

    /// expr := term (('+'|'-') term)*
    fn expr(&mut self) -> Result<(f64, bool), Error> {
        let (mut v, mut used_unit) = self.term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'+') => {
                    self.bump();
                    let (rhs, uu) = self.term()?;
                    v += rhs;
                    used_unit |= uu;
                }
                Some(b'-') => {
                    self.bump();
                    let (rhs, uu) = self.term()?;
                    v -= rhs;
                    used_unit |= uu;
                }
                _ => break,
            }
        }
        Ok((v, used_unit))
    }

    /// term := unary (('*'|'/') unary)*
    fn term(&mut self) -> Result<(f64, bool), Error> {
        let (mut v, mut used_unit) = self.unary()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'*') => {
                    self.bump();
                    let (rhs, uu) = self.unary()?;
                    v *= rhs;
                    used_unit |= uu;
                }
                Some(b'/') => {
                    self.bump();
                    let (rhs, uu) = self.unary()?;
                    v /= rhs;
                    used_unit |= uu;
                }
                _ => break,
            }
        }
        Ok((v, used_unit))
    }

    /// unary := ('+'|'-')* primary
    fn unary(&mut self) -> Result<(f64, bool), Error> {
        self.skip_ws();
        let mut sign = 1.0;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.bump();
                }
                Some(b'-') => {
                    self.bump();
                    sign = -sign;
                }
                _ => break,
            }
        }
        let (v, used_unit) = self.primary()?;
        Ok((sign * v, used_unit))
    }

    /// primary :=
    ///     NUMBER
    ///   | CONST                // pi, tau, inf, nan
    ///   | '(' expr ')'
    ///   | FUNC '(' expr ')'    // deg(...), rad(...)
    fn primary(&mut self) -> Result<(f64, bool), Error> {
        self.skip_ws();
        if let Some(c) = self.peek() {
            match c {
                b'(' => {
                    self.bump();
                    let (v, used) = self.expr()?;
                    self.skip_ws();
                    match self.bump() {
                        Some(b')') => Ok((v, used)),
                        _ => Err(self.err("expected ')'")),
                    }
                }
                c if c.is_ascii_digit() || c == b'.' => self.parse_number_or_special(),
                c if is_ident_start(c) => self.parse_ident_or_special(),
                _ => Err(self.err("expected number, constant, function, or '('")),
            }
        } else {
            Err(self.err("unexpected end of input"))
        }
    }

    /// Parse a plain number (decimal / scientific) or fall through to error.
    /// This path handles numbers that start with a digit or '.' (but not .inf/.nan).
    fn parse_number_or_special(&mut self) -> Result<(f64, bool), Error> {
        // Try YAML specials starting with '.' here: .inf / .nan (case-insensitive).
        if self.starts_ci(".inf") {
            self.i += 4;
            return Ok((f64::INFINITY, false));
        }
        if self.starts_ci(".nan") {
            self.i += 4;
            return Ok((f64::NAN, false));
        }

        let start = self.i;
        // integer part (optional if we start with '.')
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            self.i += 1;
        }
        // fraction
        if let Some(b'.') = self.peek() {
            self.i += 1;
            while let Some(c) = self.peek() {
                if !c.is_ascii_digit() {
                    break;
                }
                self.i += 1;
            }
        }
        // exponent: e[+|-]?digits
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let mut have_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    have_digit = true;
                    self.i += 1;
                } else {
                    break;
                }
            }
            if !have_digit {
                return Err(self.err("malformed exponent"));
            }
        }

        let s = &self.s[start..self.i];
        match f64::from_str(s) {
            Ok(v) => Ok((v, false)),
            Err(_) => Err(self.err("invalid float literal")),
        }
    }

    /// Parse identifiers / keywords: pi, tau, inf, nan, deg(...), rad(...)
    /// Also accepts `inf`/`nan` (without leading dot), case-insensitive.
    fn parse_ident_or_special(&mut self) -> Result<(f64, bool), Error> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if is_ident_cont(c) {
                self.i += 1;
            } else {
                break;
            }
        }
        let ident = &self.s[start..self.i];
        let ident_lc = ident.to_ascii_lowercase();

        match ident_lc.as_str() {
            "pi" => Ok((PI, false)),
            "tau" =>Ok((2.0 * PI, false)),
            "inf" => Ok((f64::INFINITY, false)),
            "nan" => Ok((f64::NAN, false)),
            "deg" | "rad" => {
                // optional whitespace before '('
                self.skip_ws();
                if self.bump() != Some(b'(') {
                    return Err(self.err("expected '(' after function name"));
                }
                let (v, _used_inner) = self.expr()?;
                self.skip_ws();
                if self.bump() != Some(b')') {
                    return Err(self.err("expected ')' after function argument"));
                }

                // Mark that a unit function was used (overrides tag semantics).
                let used_unit = true;
                return match ident_lc.as_str() {
                    "deg" => Ok((v * (PI / 180.0), used_unit)),
                    "rad" => Ok((v, used_unit)),
                    _ => unreachable!(),
                };
            }
            // If the scalar actually began with '.' and we consumed only letters,
            // handle `.inf` / `.nan` via the number path; otherwise it's an unknown ident.
            _ => Err(self.err("unknown identifier")),
        }
    }

    #[inline]
    fn starts_ci(&self, kw: &str) -> bool {
        let end = self.i + kw.len();
        if end > self.b.len() {
            return false;
        }
        self.s[self.i..end].eq_ignore_ascii_case(kw)
    }
}

#[inline]
fn is_ident_start(c: u8) -> bool {
    (c as char).is_ascii_alphabetic() || c == b'_'
}
#[inline]
fn is_ident_cont(c: u8) -> bool {
    (c as char).is_ascii_alphanumeric() || c == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;
    use crate::tags::SfTag;

    // --- helpers -----------------------------------------------------------

    // Adjust this if your Location type doesn't implement Default.
    fn loc() -> Location {
        // If your Location lacks Default, swap to your constructor, e.g. Location::new(0, 0)
        // or provide a test-only helper.
        Location::UNKNOWN
    }

    #[track_caller]
    fn assert_almost_eq_f64(actual: f64, expected: f64, eps: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan(), "expected NaN, got {actual}");
        } else if expected.is_infinite() {
            assert!(actual.is_infinite() && actual.is_sign_positive() == expected.is_sign_positive(),
                    "expected {expected}, got {actual}");
        } else {
            let diff = (actual - expected).abs();
            assert!(
                diff <= eps,
                "expected {expected} ± {eps}, got {actual} (diff {diff})"
            );
        }
    }

    #[track_caller]
    fn assert_ok64(s: &str, tag: SfTag, expected: f64) {
        let v: f64 = parse_yaml12_float_angle_converting::<f64>(s, loc(), tag).unwrap();
        assert_almost_eq_f64(v, expected, 1e-12);
    }

    #[track_caller]
    fn assert_ok32(s: &str, tag: SfTag, expected: f32) {
        let v: f32 = parse_yaml12_float_angle_converting::<f32>(s, loc(), tag).unwrap();
        let diff = (v - expected).abs();
        assert!(diff <= 1e-6, "expected {expected} ± 1e-6, got {v} (diff {diff})");
    }

    #[track_caller]
    fn assert_err(s: &str, tag: SfTag) {
        let r: Result<f64, Error> = parse_yaml12_float_angle_converting::<f64>(s, loc(), tag);
        assert!(r.is_err(), "expected error for `{s}`, got {:?}", r.ok());
    }

    // --- plain numbers -----------------------------------------------------

    #[test]
    fn plain_numbers() {
        assert_ok64("0.15", SfTag::Radians, 0.15);
        assert_ok64("-1", SfTag::Radians, -1.0);
        assert_ok64("1e-3", SfTag::Radians, 1e-3);
        assert_ok64(".5", SfTag::Radians, 0.5);
        assert_ok64("10.", SfTag::Radians, 10.0);
        assert_ok64("  42  ", SfTag::Radians, 42.0);
    }

    // --- YAML specials (.inf/.nan and inf/nan) -----------------------------

    #[test]
    fn yaml_specials_dot_forms() {
        assert_ok64(".inf", SfTag::Radians, f64::INFINITY);
        assert_ok64("+.inf", SfTag::Radians, f64::INFINITY);
        assert_ok64("-.inf", SfTag::Radians, f64::NEG_INFINITY);
        assert!(parse_yaml12_float_angle_converting::<f64>(".nan", loc(), SfTag::Radians)
            .unwrap()
            .is_nan());
        assert!(parse_yaml12_float_angle_converting::<f64>("-.NaN", loc(), SfTag::Radians)
            .unwrap()
            .is_nan());
        assert!(parse_yaml12_float_angle_converting::<f64>("+.nAn", loc(), SfTag::Radians)
            .unwrap()
            .is_nan());
    }

    #[test]
    fn yaml_specials_ident_forms() {
        assert_ok64("inf", SfTag::Radians, f64::INFINITY);
        assert_ok64("+INF", SfTag::Radians, f64::INFINITY);
        assert_ok64("-InF", SfTag::Radians, f64::NEG_INFINITY);
        assert!(parse_yaml12_float_angle_converting::<f64>("nan", loc(), SfTag::Radians)
            .unwrap()
            .is_nan());
        assert!(parse_yaml12_float_angle_converting::<f64>("-NaN", loc(), SfTag::Radians)
            .unwrap()
            .is_nan());
    }

    // --- constants: pi / tau (case-insensitive) ----------------------------

    #[test]
    fn constants_case_insensitive() {
        assert_ok64("pi", SfTag::Radians, PI);
        assert_ok64("PI", SfTag::Radians, PI);
        assert_ok64("tau", SfTag::Radians, 2.0 * PI);
        assert_ok64("TAU", SfTag::Radians, 2.0 * PI);
    }

    // --- arithmetic & precedence -------------------------------------------

    #[test]
    fn expressions_precedence_and_parentheses() {
        assert_ok64("2*pi", SfTag::Radians, 2.0 * PI);
        assert_ok64("pi/2", SfTag::Radians, PI / 2.0);
        // 1 + 2*(3 - 4/5) = 1 + 2*(3 - 0.8) = 1 + 2*2.2 = 5.4
        assert_ok64("1 + 2*(3 - 4/5)", SfTag::Radians, 5.4);
        // chained ops and whitespace
        assert_ok64("  3 + 4*2 / (1 - 5) ", SfTag::Radians, 3.0 + 4.0 * 2.0 / (1.0 - 5.0));
    }

    #[test]
    fn unary_signs() {
        assert_ok64("--1", SfTag::Radians, 1.0);
        assert_ok64("-- 1", SfTag::Radians, 1.0);
        assert_ok64("3--2", SfTag::Radians, 5.0);
        assert_ok64("3-+2", SfTag::Radians, 1.0);
    }

    // --- conversion functions: deg(...) and rad(...) -----------------------

    #[test]
    fn conversion_functions_basic() {
        assert_ok64("deg(180)", SfTag::Radians, PI);
        assert_ok64("deg(90+45)", SfTag::Radians, 135.0 * PI / 180.0);
        assert_ok64("rad(2*pi)", SfTag::Radians, 2.0 * PI);
        // whitespace tolerance
        assert_ok64("deg ( 180 )", SfTag::Radians, PI);
        assert_ok64("rad( (pi) )", SfTag::Radians, PI);
    }

    #[test]
    fn conversion_functions_nested_exprs() {
        assert_ok64("deg( (45 + 45) )", SfTag::Radians, PI / 2.0);
        assert_ok64("rad( 2 * (pi/2) )", SfTag::Radians, 2.0 * (PI / 2.0));
    }

    // --- tag interaction (no function used) --------------------------------

    #[test]
    fn tags_without_functions() {
        // Degrees tag converts to radians
        assert_ok64("180", SfTag::Degrees, PI);
        assert_ok64("-90", SfTag::Degrees, -PI / 2.0);
        // Radians tag leaves as-is
        assert_ok64("3.141592653589793", SfTag::Radians, PI);
        // Ensure expressions with tags are handled (no unit function, so tag applies)
        assert_ok64("2*pi", SfTag::Degrees, (2.0 * PI) * (PI / 180.0));
    }

    // --- precedence: function wins over tag --------------------------------

    #[test]
    fn function_overrides_tag() {
        // Tag should be ignored when unit function is present
        assert_ok64("deg(180)", SfTag::Degrees, PI);
        assert_ok64("deg(180)", SfTag::Radians, PI);
        assert_ok64("rad(2*pi)", SfTag::Degrees, 2.0 * PI);
        assert_ok64("rad(2*pi)", SfTag::Radians, 2.0 * PI);
    }

    // --- numeric edge cases -------------------------------------------------

    #[test]
    fn division_by_zero_and_nan_propagation() {
        // 1/0 => +inf in IEEE-754
        let v: f64 = parse_yaml12_float_angle_converting("1/0", loc(), SfTag::Radians).unwrap();
        assert!(v.is_infinite() && v.is_sign_positive(), "expected +inf, got {v}");
        // nan propagation in expression
        let v2: f64 = parse_yaml12_float_angle_converting("nan + 1", loc(), SfTag::Radians).unwrap();
        assert!(v2.is_nan(), "expected NaN, got {v2}");
    }

    #[test]
    fn exponents_and_formats() {
        assert_ok64("1e3", SfTag::Radians, 1000.0);
        assert_ok64("1E-3", SfTag::Radians, 1e-3);
        assert_ok64(".5e+1", SfTag::Radians, 5.0);
        // 1e400 overflows to +inf in Rust f64 FromStr
        assert_ok64("1e400", SfTag::Radians, f64::INFINITY);
    }

    // --- f32 generic path sanity -------------------------------------------

    #[test]
    fn generic_f32_output() {
        assert_ok32("deg(180)", SfTag::Radians, core::f32::consts::PI);
        assert_ok32("rad(2*pi)", SfTag::Radians, 2.0 * core::f32::consts::PI);
    }

    // --- trailing and lexical errors ---------------------------------------

    #[test]
    fn errors_trailing_and_lexical() {
        // trailing garbage
        assert_err("1 2", SfTag::Radians);
        assert_err("1pi", SfTag::Radians);
        // malformed exponent
        assert_err("1e", SfTag::Radians);
        assert_err("1e+", SfTag::Radians);
        // invalid literal
        assert_err(".", SfTag::Radians);
        // unknown identifier
        assert_err("foo", SfTag::Radians);
    }

    // --- parenthesis & function call errors --------------------------------

    #[test]
    fn errors_parentheses_and_calls() {
        assert_err("(", SfTag::Radians);
        assert_err("(1+2", SfTag::Radians);
        assert_err("deg(90", SfTag::Radians);
        assert_err("deg 90)", SfTag::Radians); // missing '(' after function name
        assert_err("rad)", SfTag::Radians);
        // empty function argument
        assert_err("deg()", SfTag::Radians);
    }
}
