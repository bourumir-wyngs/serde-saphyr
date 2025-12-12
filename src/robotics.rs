/// Parse and evaluate a YAML 1.2 float scalar with robotics-style angle support.
///
/// This function accepts plain numbers, expressions, and unit functions,
/// evaluates them, and returns the numeric value (converted to radians when
/// applicable). It uses a small recursive-descent evaluator supporting `+ - * /`
/// with parentheses and constants.
///
/// # Supported features
/// - YAML 1.2 float forms: `0.15`, `1e-3`, `.inf`, `.nan`, etc. (case-insensitive)
/// - Digit separators: underscores between digits are allowed in numbers and exponents:
///   `1_000.0`, `0.1_5`, `1e1_0` (strict: underscores must be between digits)
/// - Constants: `pi`, `tau`, `inf`, `nan`
/// - Expressions: `2*pi`, `1 + 2*(3 - 4/5)`, `pi/2`
/// - Unit functions:
///     - `deg(<expr>)` — interpret as degrees, convert to radians
///     - `rad(<expr>)` — interpret as radians (no conversion)
/// - Sexagesimal degrees: `hh:mm[:ss[.frac]]` (e.g., `12:30`, `-0:30:30.5`), converted to radians
///
/// # Tag interaction
/// - If no `deg`/`rad` is used, `SfTag::Degrees` converts to radians.
/// - `SfTag::Radians` leaves values as-is.
/// - Unitized inputs (`deg(...)`, `rad(...)`, **sexagesimal**) override tag-based conversion.
/// - Safety rule: mixing unitized inputs with bare terms under `SfTag::Degrees` is rejected
///   to avoid ambiguous semantics. Wrap bare terms explicitly with `deg(...)` or `rad(...)`,
///   or remove the tag.
///
/// # Errors
/// Returns a descriptive [`Error`] with [`Location`] for malformed syntax,
/// unbalanced parentheses, or unknown identifiers.
/// Also rejects invalid underscore placement and excessive recursion or digit counts.
///
use core::f64::consts::PI;
use core::str::FromStr;


// small constants / guards
const DEG2RAD: f64 = PI / 180.0;
const MAX_EXPR_DEPTH: u32 = 256; // guard against deeply nested parentheses/functions
const MAX_NUM_DIGITS: usize = 1_000_000; // cap digits in a single numeric token (DoS mitigation)

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

// Evaluator result: (value, used_unitized, saw_plain_outside)
type Eval = (f64, bool, bool);

/// Parse/evaluate expression with angle conversion semantics.
pub(crate) fn parse_yaml12_float_angle_converting<T>(
    s: &str,
    location: Location,
    tag: SfTag,
) -> Result<T, Error>
where
    T: FromF64,
{
    let mut p = Parser::new(s, location, tag);
    p.skip_ws();
    let (mut value, used_unit, saw_plain) = p.expr()?; // parse whole expression
    p.skip_ws();
    if !p.eof() {
        return Err(p.err("unexpected trailing characters in scalar"));
    }

    // Tag-based conversion only if no unitized constructs were used.
    if !used_unit {
        match tag {
            SfTag::Degrees => value *= DEG2RAD,
            SfTag::Radians => { /* already radians */ }
            _ => { /* ignore other tags for floats */ }
        }
    } else {
        // Safety: prevent ambiguous mixing under Degrees tag.
        if matches!(tag, SfTag::Degrees) && saw_plain {
            return Err(p.err(
                "ambiguous mix of unitized values and Degrees tag: \
                 wrap bare terms with deg(...) or rad(...), or remove the tag",
            ));
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
    depth: u32,
    tag: SfTag,
    sexagesimal_is_time: bool,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str, loc: Location, tag: SfTag) -> Self {
        // Default: time semantics for hh:mm[:ss] → seconds
        Self {
            s,
            b: s.as_bytes(),
            i: 0,
            loc,
            depth: 0,
            tag,
            sexagesimal_is_time: true,
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
        Error::HookError {
            msg: msg.to_string(),
            location: self.loc,
        }
    }
    #[inline]
    fn enter(&mut self) -> Result<(), Error> {
        if self.depth >= MAX_EXPR_DEPTH {
            return Err(self.err("expression too deeply nested"));
        }
        self.depth += 1;
        Ok(())
    }
    #[inline]
    fn exit(&mut self) {
        debug_assert!(self.depth > 0);
        self.depth -= 1;
    }

    /// expr := term (('+'|'-') term)*
    fn expr(&mut self) -> Result<Eval, Error> {
        let (mut v, mut used_unit, mut saw_plain) = self.term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'+') => {
                    self.bump();
                    let (rhs, uu, sp) = self.term()?;
                    v += rhs;
                    used_unit |= uu;
                    saw_plain |= sp;
                }
                Some(b'-') => {
                    self.bump();
                    let (rhs, uu, sp) = self.term()?;
                    v -= rhs;
                    used_unit |= uu;
                    saw_plain |= sp;
                }
                _ => break,
            }
        }
        Ok((v, used_unit, saw_plain))
    }

    /// term := unary (('*'|'/') unary)*
    fn term(&mut self) -> Result<Eval, Error> {
        let (mut v, mut used_unit, mut saw_plain) = self.unary()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'*') => {
                    self.bump();
                    let (rhs, uu, sp) = self.unary()?;
                    v *= rhs;
                    used_unit |= uu;
                    saw_plain |= sp;
                }
                Some(b'/') => {
                    self.bump();
                    let (rhs, uu, sp) = self.unary()?;
                    v /= rhs;
                    used_unit |= uu;
                    saw_plain |= sp;
                }
                _ => break,
            }
        }
        Ok((v, used_unit, saw_plain))
    }

    /// unary := ('+'|'-')* primary
    fn unary(&mut self) -> Result<Eval, Error> {
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
        let (v, used_unit, saw_plain) = self.primary()?;
        Ok((sign * v, used_unit, saw_plain))
    }

    /// primary :=
    ///     NUMBER
    ///   | SEXAGESIMAL          // hh:mm[:ss[.frac]]  (degrees)
    ///   | CONST                // pi, tau, inf, nan
    ///   | '(' expr ')'
    ///   | FUNC '(' expr ')'    // deg(...), rad(...)
    fn primary(&mut self) -> Result<Eval, Error> {
        self.skip_ws();
        if let Some(c) = self.peek() {
            match c {
                b'(' => {
                    self.bump();
                    self.enter()?;
                    let r = self.expr();
                    self.exit();
                    let (v, used, plain) = r?;
                    self.skip_ws();
                    match self.bump() {
                        Some(b')') => Ok((v, used, plain)),
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

    /// Parse a number/special/sexagesimal.
    fn parse_number_or_special(&mut self) -> Result<Eval, Error> {
        // `.inf` / `.nan` (case-insensitive)
        if self.starts_ci(".inf") {
            self.i += 4;
            return Ok((f64::INFINITY, false, true));
        }
        if self.starts_ci(".nan") {
            self.i += 4;
            return Ok((f64::NAN, false, true));
        }

        // Sexagesimal look-ahead (starts with digits or '.'? only digits make sense here):
        if let Some(res) = self.try_parse_sexagesimal()? {
            return Ok(res);
        }

        // Regular float with optional underscores
        let start = self.i;
        let mut digits_seen: usize = 0; // count digits only
        let mut buf = Vec::<u8>::with_capacity(32);

        // integer part (optional if we start with '.')
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                digits_seen += 1;
                buf.push(c);
                self.i += 1;
            } else if c == b'_' {
                // underscore must be between digits
                let next = self.b.get(self.i + 1).copied();
                let prev_is_digit = self.i > start && self.b[self.i - 1].is_ascii_digit();
                if !prev_is_digit || !matches!(next, Some(nc) if nc.is_ascii_digit()) {
                    return Err(self.err("invalid underscore placement in number"));
                }
                self.i += 1; // skip underscore
            } else {
                break;
            }
            if digits_seen > MAX_NUM_DIGITS {
                return Err(self.err("too many digits in numeric literal"));
            }
        }

        // fraction
        if let Some(b'.') = self.peek() {
            buf.push(b'.');
            self.i += 1;
            let frac_start_i = self.i;
            let mut had_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    had_digit = true;
                    digits_seen += 1;
                    buf.push(c);
                    self.i += 1;
                } else if c == b'_' {
                    // underscore must be between digits
                    let next = self.b.get(self.i + 1).copied();
                    let prev_is_digit =
                        self.i > frac_start_i && self.b[self.i - 1].is_ascii_digit();
                    if !prev_is_digit || !matches!(next, Some(nc) if nc.is_ascii_digit()) {
                        return Err(self.err("invalid underscore placement in fraction"));
                    }
                    self.i += 1; // skip underscore
                } else {
                    break;
                }
                if digits_seen > MAX_NUM_DIGITS {
                    return Err(self.err("too many digits in numeric literal"));
                }
            }
            // allow "10." (no digits after dot)
            let _ = had_digit;
        }

        // exponent: e[+|-]?digits (digits may contain underscores between digits)
        if matches!(self.peek(), Some(b'e' | b'E')) {
            // Safe to unwrap previously because we checked with peek; avoid unwrap by matching again.
            if let Some(c) = self.bump() {
                buf.push(c);
            } else {
                return Err(self.err("expected exponent marker"));
            }
            if matches!(self.peek(), Some(b'+' | b'-')) {
                if let Some(sign) = self.bump() {
                    buf.push(sign);
                } else {
                    return Err(self.err("expected sign after exponent marker"));
                }
            }
            let exp_start = self.i;
            let mut have_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    have_digit = true;
                    digits_seen += 1;
                    buf.push(c);
                    self.i += 1;
                } else if c == b'_' {
                    let next = self.b.get(self.i + 1).copied();
                    let prev_is_digit = self.i > exp_start && self.b[self.i - 1].is_ascii_digit();
                    if !prev_is_digit || !matches!(next, Some(nc) if nc.is_ascii_digit()) {
                        return Err(self.err("invalid underscore placement in exponent"));
                    }
                    self.i += 1; // skip underscore
                } else {
                    break;
                }
                if digits_seen > MAX_NUM_DIGITS {
                    return Err(self.err("too many digits in numeric literal"));
                }
            }
            if !have_digit {
                return Err(self.err("malformed exponent"));
            }
        }

        if buf.is_empty() {
            // no underscores -> use slice directly
            let s = &self.s[start..self.i];
            match f64::from_str(s) {
                Ok(v) => Ok((v, false, true)),
                Err(_) => Err(self.err("invalid float literal")),
            }
        } else {
            // underscores removed in buf
            match core::str::from_utf8(&buf)
                .map_err(|_| self.err("invalid utf-8 in numeric literal"))
                .and_then(|s| f64::from_str(s).map_err(|_| self.err("invalid float literal")))
            {
                Ok(v) => Ok((v, false, true)),
                Err(_) => Err(self.err("invalid float literal")),
            }
        }
    }

    /// Parse identifiers / keywords: pi, tau, inf, nan, deg(...), rad(...)
    fn parse_ident_or_special(&mut self) -> Result<Eval, Error> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if is_ident_cont(c) {
                self.i += 1;
            } else {
                break;
            }
        }
        let ident = &self.s[start..self.i];

        // Case-insensitive match without allocating a lowercase copy
        if ident.eq_ignore_ascii_case("pi") {
            return Ok((PI, false, true));
        }
        if ident.eq_ignore_ascii_case("tau") {
            return Ok((2.0 * PI, false, true));
        }
        if ident.eq_ignore_ascii_case("inf") {
            return Ok((f64::INFINITY, false, true));
        }
        if ident.eq_ignore_ascii_case("nan") {
            return Ok((f64::NAN, false, true));
        }
        if ident.eq_ignore_ascii_case("deg") || ident.eq_ignore_ascii_case("rad") {
            self.skip_ws();
            if self.bump() != Some(b'(') {
                return Err(self.err("expected '(' after function name"));
            }
            // Inside unit functions, treat sexagesimal as degrees (angle) rather than time.
            let old_mode = self.sexagesimal_is_time;
            self.sexagesimal_is_time = false;
            self.enter()?;
            let r = self.expr();
            self.exit();
            self.sexagesimal_is_time = old_mode;
            let (v, _used_inner, _plain_inner) = r?;
            self.skip_ws();
            if self.bump() != Some(b')') {
                return Err(self.err("expected ')' after function argument"));
            }

            let used_unit = true;
            if ident.eq_ignore_ascii_case("deg") {
                Ok((v * DEG2RAD, used_unit, false))
            } else {
                Ok((v, used_unit, false))
            }
        } else {
            Err(self.err("unknown identifier"))
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

    /// Attempt to parse a sexagesimal literal: hh:mm[:ss[.frac]]
    /// Returns Ok(Some(...)) if matched, Ok(None) if not a sexagesimal start.
    fn try_parse_sexagesimal(&mut self) -> Result<Option<Eval>, Error> {
        let save = self.i;

        // We allow only a digits/underscores run immediately followed by ':' to enter this path.
        let mut j = self.i;
        let mut saw_digit = false;
        let mut last_underscore = false;
        while let Some(c) = self.b.get(j).copied() {
            if c.is_ascii_digit() {
                saw_digit = true;
                last_underscore = false;
                j += 1;
            } else if c == b'_' {
                if !saw_digit || last_underscore {
                    break;
                }
                last_underscore = true;
                j += 1;
            } else {
                break;
            }
        }
        if !saw_digit || last_underscore {
            return Ok(None);
        }
        if self.b.get(j).copied() != Some(b':') {
            return Ok(None);
        }

        // degrees (D in D:M[:S[.frac]])
        let (deg_whole, d1) = self.read_uint_unders_to_f64()?;
        if self.bump() != Some(b':') {
            self.i = save;
            return Ok(None);
        }

        // minutes
        let (mins_u, d2) = self.read_uint_unders_to_u32()?;
        if mins_u > 59 {
            return Err(self.err("minutes out of range in sexagesimal literal"));
        }

        let mut secs: f64 = 0.0;
        let mut total_digits = d1 + d2;

        if let Some(b':') = self.peek() {
            self.bump();
            let (secs_u, d3) = self.read_uint_unders_to_u32()?;
            if secs_u > 59 {
                return Err(self.err("seconds out of range in sexagesimal literal"));
            }
            total_digits += d3;
            secs = secs_u as f64;

            if let Some(b'.') = self.peek() {
                self.bump();
                let (frac, df) = self.read_frac_part_unders()?;
                total_digits += df;
                secs += frac;
            }
        }

        if total_digits > MAX_NUM_DIGITS {
            return Err(self.err("too many digits in sexagesimal literal"));
        }

        // Interpret sexagesimal based on current mode and tag:
        // - Default/time or explicit TimeStamp tag -> total seconds
        // - Inside unit functions (sexagesimal_is_time == false) -> degrees numeric (wrapping unit converts)
        // - Top-level with Degrees/Radians tag -> treat as angle and produce radians directly
        if self.sexagesimal_is_time {
            if matches!(self.tag, SfTag::Degrees | SfTag::Radians) {
                let degrees = deg_whole + (mins_u as f64) / 60.0 + secs / 3600.0;
                Ok(Some((degrees * DEG2RAD, true, false)))
            } else {
                // Time mode: hh:mm[:ss[.frac]] → total seconds
                let total_seconds = deg_whole * 3600.0 + (mins_u as f64) * 60.0 + secs;
                Ok(Some((total_seconds, true, false)))
            }
        } else if matches!(self.tag, SfTag::TimeStamp) {
            // Explicit time tag forces time semantics even inside functions (unlikely combo)
            let total_seconds = deg_whole * 3600.0 + (mins_u as f64) * 60.0 + secs;
            Ok(Some((total_seconds, true, false)))
        } else {
            // Angle mode (inside unit functions): interpret as degrees numeric; conversion is handled by the wrapping unit (deg()/rad()).
            let degrees = deg_whole + (mins_u as f64) / 60.0 + secs / 3600.0;
            Ok(Some((degrees, true, false)))
        }
    }

    /// Read unsigned integer with underscores to f64. Returns (value, digit_count).
    fn read_uint_unders_to_f64(&mut self) -> Result<(f64, usize), Error> {
        let mut v: f64 = 0.0;
        let mut digits: usize = 0;
        let start = self.i;
        let mut prev_is_digit = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                v = v * 10.0 + (c - b'0') as f64;
                self.i += 1;
                digits += 1;
                prev_is_digit = true;
            } else if c == b'_' {
                let next = self.b.get(self.i + 1).copied();
                if !prev_is_digit || !matches!(next, Some(nc) if nc.is_ascii_digit()) {
                    return Err(self.err("invalid underscore placement in integer field"));
                }
                self.i += 1;
                prev_is_digit = false;
            } else {
                break;
            }
            if digits > MAX_NUM_DIGITS {
                return Err(self.err("too many digits in integer field"));
            }
        }
        if digits == 0 {
            return Err(self.err("expected digits"));
        }
        debug_assert!(self.i > start);
        Ok((v, digits))
    }

    /// Read unsigned integer with underscores to u32. Returns (value, digit_count).
    fn read_uint_unders_to_u32(&mut self) -> Result<(u32, usize), Error> {
        let (v_f, d) = self.read_uint_unders_to_f64()?;
        if v_f > u32::MAX as f64 {
            return Err(self.err("numeric field too large"));
        }
        Ok((v_f as u32, d))
    }

    /// Read fractional digits with underscores after '.' → (fraction_value, digit_count).
    fn read_frac_part_unders(&mut self) -> Result<(f64, usize), Error> {
        let mut num: f64 = 0.0;
        let mut scale: f64 = 1.0;
        let mut digits: usize = 0;
        let mut prev_is_digit = false;
        const MAX_FRAC_DIGITS: usize = 18; // enough for f64 precision
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                if digits < MAX_FRAC_DIGITS {
                    num = num * 10.0 + (c - b'0') as f64;
                    scale *= 10.0;
                }
                self.i += 1;
                digits += 1;
                prev_is_digit = true;
            } else if c == b'_' {
                let next = self.b.get(self.i + 1).copied();
                if !prev_is_digit || !matches!(next, Some(nc) if nc.is_ascii_digit()) {
                    return Err(self.err("invalid underscore placement in fraction"));
                }
                self.i += 1;
                prev_is_digit = false;
            } else {
                break;
            }
            if digits > MAX_NUM_DIGITS {
                return Err(self.err("too many digits in fraction"));
            }
        }
        if digits == 0 {
            return Err(self.err("expected digits after decimal point"));
        }
        Ok((num / scale, digits))
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
    use crate::{Error, Location};
use crate::tags::SfTag;
    use core::f64::consts::PI;
    use crate::robotics::{parse_yaml12_float_angle_converting, DEG2RAD};

    // helpers
    fn loc() -> Location {
        Location::UNKNOWN
    }

    #[track_caller]
    fn assert_almost_eq_f64(actual: f64, expected: f64, eps: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan(), "expected NaN, got {actual}");
        } else if expected.is_infinite() {
            assert!(
                actual.is_infinite() && actual.is_sign_positive() == expected.is_sign_positive(),
                "expected {expected}, got {actual}"
            );
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
        assert!(
            diff <= 1e-6,
            "expected {expected} ± 1e-6, got {v} (diff {diff})"
        );
    }

    #[track_caller]
    fn assert_err(s: &str, tag: SfTag) {
        let r: Result<f64, Error> = parse_yaml12_float_angle_converting::<f64>(s, loc(), tag);
        assert!(r.is_err(), "expected error for `{s}`, got {:?}", r.ok());
    }

    // plain numbers
    #[test]
    fn plain_numbers() {
        assert_ok64("0.15", SfTag::Radians, 0.15);
        assert_ok64("-1", SfTag::Radians, -1.0);
        assert_ok64("1e-3", SfTag::Radians, 1e-3);
        assert_ok64(".5", SfTag::Radians, 0.5);
        assert_ok64("10.", SfTag::Radians, 10.0);
        assert_ok64("  42  ", SfTag::Radians, 42.0);
    }

    // YAML specials
    #[test]
    fn yaml_specials_dot_forms() {
        assert_ok64(".inf", SfTag::Radians, f64::INFINITY);
        assert_ok64("+.inf", SfTag::Radians, f64::INFINITY);
        assert_ok64("-.inf", SfTag::Radians, f64::NEG_INFINITY);
        assert!(
            parse_yaml12_float_angle_converting::<f64>(".nan", loc(), SfTag::Radians)
                .unwrap()
                .is_nan()
        );
        assert!(
            parse_yaml12_float_angle_converting::<f64>("-.NaN", loc(), SfTag::Radians)
                .unwrap()
                .is_nan()
        );
        assert!(
            parse_yaml12_float_angle_converting::<f64>("+.nAn", loc(), SfTag::Radians)
                .unwrap()
                .is_nan()
        );
    }

    #[test]
    fn yaml_specials_ident_forms() {
        assert_ok64("inf", SfTag::Radians, f64::INFINITY);
        assert_ok64("+INF", SfTag::Radians, f64::INFINITY);
        assert_ok64("-InF", SfTag::Radians, f64::NEG_INFINITY);
        assert!(
            parse_yaml12_float_angle_converting::<f64>("nan", loc(), SfTag::Radians)
                .unwrap()
                .is_nan()
        );
        assert!(
            parse_yaml12_float_angle_converting::<f64>("-NaN", loc(), SfTag::Radians)
                .unwrap()
                .is_nan()
        );
    }

    // constants
    #[test]
    fn constants_case_insensitive() {
        assert_ok64("pi", SfTag::Radians, PI);
        assert_ok64("PI", SfTag::Radians, PI);
        assert_ok64("tau", SfTag::Radians, 2.0 * PI);
        assert_ok64("TAU", SfTag::Radians, 2.0 * PI);
    }

    // arithmetic / precedence
    #[test]
    fn expressions_precedence_and_parentheses() {
        assert_ok64("2*pi", SfTag::Radians, 2.0 * PI);
        assert_ok64("pi/2", SfTag::Radians, PI / 2.0);
        assert_ok64("1 + 2*(3 - 4/5)", SfTag::Radians, 5.4);
        assert_ok64(
            "  3 + 4*2 / (1 - 5) ",
            SfTag::Radians,
            3.0 + 4.0 * 2.0 / (1.0 - 5.0),
        );
    }

    // underscores
    #[test]
    fn underscores_in_numbers() {
        assert_ok64("1_000.0", SfTag::Radians, 1000.0);
        assert_ok64("0.1_5", SfTag::Radians, 0.15);
        assert_ok64("1e1_0", SfTag::Radians, 1e10);
        assert_err("1__0", SfTag::Radians);
        assert_err("1_", SfTag::Radians);
        assert_err("1._0", SfTag::Radians);
        assert_err("1e_10", SfTag::Radians);
    }

    // sexagesimal
    #[test]
    fn sexagesimal_basic() {
        // Default semantics: time → total seconds
        assert_ok64("90:0:0", SfTag::None, 90.0 * 3600.0);
        assert_ok64("180:0", SfTag::None, 180.0 * 3600.0);
        // -0:30:30.5 => -(0*3600 + 30*60 + 30.5) seconds
        let expected = -(0.0 * 3600.0 + 30.0 * 60.0 + 30.5);
        let v: f64 =
            parse_yaml12_float_angle_converting(" -0:30:30.5 ", loc(), SfTag::None).unwrap();
        assert_almost_eq_f64(v, expected, 1e-12);
        // Angle via sexagesimal must be explicit using deg(...)
        let angle: f64 =
            parse_yaml12_float_angle_converting("deg(1:2:3)", loc(), SfTag::Radians).unwrap();
        let degs = 1.0 + 2.0 / 60.0 + 3.0 / 3600.0;
        assert_almost_eq_f64(angle, degs * DEG2RAD, 1e-12);
    }

    // unary
    #[test]
    fn unary_signs() {
        assert_ok64("--1", SfTag::Radians, 1.0);
        assert_ok64("-- 1", SfTag::Radians, 1.0);
        assert_ok64("3--2", SfTag::Radians, 5.0);
        assert_ok64("3-+2", SfTag::Radians, 1.0);
    }

    // unit functions
    #[test]
    fn conversion_functions_basic() {
        assert_ok64("deg(180)", SfTag::Radians, PI);
        assert_ok64("deg(90+45)", SfTag::Radians, 135.0 * PI / 180.0);
        assert_ok64("rad(2*pi)", SfTag::Radians, 2.0 * PI);
        assert_ok64("deg ( 180 )", SfTag::Radians, PI);
        assert_ok64("rad( (pi) )", SfTag::Radians, PI);
    }

    #[test]
    fn conversion_functions_nested_exprs() {
        assert_ok64("deg( (45 + 45) )", SfTag::Radians, PI / 2.0);
        assert_ok64("rad( 2 * (pi/2) )", SfTag::Radians, 2.0 * (PI / 2.0));
    }

    // tag interaction (no function used)
    #[test]
    fn tags_without_functions() {
        assert_ok64("180", SfTag::Degrees, PI);
        assert_ok64("-90", SfTag::Degrees, -PI / 2.0);
        assert_ok64("3.141592653589793", SfTag::Radians, PI);
        assert_ok64("2*pi", SfTag::Degrees, (2.0 * PI) * (PI / 180.0));
    }

    // precedence: function wins over tag
    #[test]
    fn function_overrides_tag() {
        assert_ok64("deg(180)", SfTag::Degrees, PI);
        assert_ok64("deg(180)", SfTag::Radians, PI);
        assert_ok64("rad(2*pi)", SfTag::Degrees, 2.0 * PI);
        assert_ok64("rad(2*pi)", SfTag::Radians, 2.0 * PI);
    }

    // ambiguity rule
    #[test]
    fn mixed_units_with_degrees_tag_errors() {
        assert_err("30:0:0 + 90", SfTag::Degrees);
        assert_err("deg(90) + 90", SfTag::Degrees);
        assert_err("rad(1) + pi/2", SfTag::Degrees);
    }

    #[test]
    fn mixed_units_with_radians_tag_is_ok() {
        // Angle via sexagesimal must be explicit when mixing units
        assert_ok64(
            "deg(30:0:0) + 0.001",
            SfTag::Radians,
            30.0 * DEG2RAD + 0.001,
        );
    }

    // numeric edge cases
    #[test]
    fn division_by_zero_and_nan_propagation() {
        let v: f64 = parse_yaml12_float_angle_converting("1/0", loc(), SfTag::Radians).unwrap();
        assert!(
            v.is_infinite() && v.is_sign_positive(),
            "expected +inf, got {v}"
        );
        let v2: f64 =
            parse_yaml12_float_angle_converting("nan + 1", loc(), SfTag::Radians).unwrap();
        assert!(v2.is_nan(), "expected NaN, got {v2}");
    }

    #[test]
    fn exponents_and_formats() {
        assert_ok64("1e3", SfTag::Radians, 1000.0);
        assert_ok64("1E-3", SfTag::Radians, 1e-3);
        assert_ok64(".5e+1", SfTag::Radians, 5.0);
        assert_ok64("1e400", SfTag::Radians, f64::INFINITY);
    }

    // f32 generic path
    #[test]
    fn generic_f32_output() {
        assert_ok32("deg(180)", SfTag::Radians, core::f32::consts::PI);
        assert_ok32("rad(2*pi)", SfTag::Radians, 2.0 * core::f32::consts::PI);
    }

    // lexical/trailing errors
    #[test]
    fn errors_trailing_and_lexical() {
        assert_err("1 2", SfTag::Radians);
        assert_err("1pi", SfTag::Radians);
        assert_err("1e", SfTag::Radians);
        assert_err("1e+", SfTag::Radians);
        assert_err(".", SfTag::Radians);
        assert_err("foo", SfTag::Radians);
        assert_err("10:60", SfTag::Radians);
    }

    // paren / calls errors
    #[test]
    fn errors_parentheses_and_calls() {
        assert_err("(", SfTag::Radians);
        assert_err("(1+2", SfTag::Radians);
        assert_err("deg(90", SfTag::Radians);
        assert_err("deg 90)", SfTag::Radians);
        assert_err("rad)", SfTag::Radians);
        assert_err("deg()", SfTag::Radians);
    }
}
