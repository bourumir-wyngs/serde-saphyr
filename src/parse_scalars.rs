use crate::sf_serde::{Error, Location};

/// Parse a YAML 1.1 boolean from a &str (handles the "Norway problem").
///
/// Accepted TRUE literals (case-insensitive): "y", "yes", "true", "on"
/// Accepted FALSE literals (case-insensitive): "n", "no", "false", "off"
///
/// Returns:
/// - Ok(true/false) on success
/// - Err(...) if the input is not a YAML 1.1 boolean literal
pub(crate) fn parse_yaml11_bool(s: &str) -> Result<bool, String> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("y")
        || t.eq_ignore_ascii_case("on")
    {
        Ok(true)
    } else if t.eq_ignore_ascii_case("false")
        || t.eq_ignore_ascii_case("no")
        || t.eq_ignore_ascii_case("n")
        || t.eq_ignore_ascii_case("off")
    {
        Ok(false)
    } else {
        Err(format!("invalid YAML 1.1 bool: `{}`", s))
    }
}

fn parse_digits_u128(digits: &str, radix: u32) -> Option<u128> {
    let mut val: u128 = 0;
    let mut saw = false;
    for b in digits.as_bytes() {
        match *b {
            b'_' => continue,
            b'0'..=b'9' => {
                let d = (b - b'0') as u32;
                if d >= radix { return None; }
                val = val.checked_mul(radix as u128)?;
                val = val.checked_add(d as u128)?;
                saw = true;
            }
            b'a'..=b'f' if radix > 10 => {
                let d = 10 + (b - b'a') as u32;
                if d >= radix { return None; }
                val = val.checked_mul(radix as u128)?;
                val = val.checked_add(d as u128)?;
                saw = true;
            }
            b'A'..=b'F' if radix > 10 => {
                let d = 10 + (b - b'A') as u32;
                if d >= radix { return None; }
                val = val.checked_mul(radix as u128)?;
                val = val.checked_add(d as u128)?;
                saw = true;
            }
            _ => return None,
        }
    }
    if saw { Some(val) } else { None }
}

fn parse_decimal_unsigned_u128(digits: &str) -> Option<u128> {
    let mut val: u128 = 0;
    let mut saw = false;
    for b in digits.as_bytes() {
        match *b {
            b'_' => continue,
            b'0'..=b'9' => {
                let d = (b - b'0') as u128;
                val = val.checked_mul(10)?;
                val = val.checked_add(d)?;
                saw = true;
            }
            _ => return None,
        }
    }
    if saw { Some(val) } else { None }
}

fn parse_decimal_signed_i128(digits: &str, neg: bool) -> Option<i128> {
    if neg {
        // Accumulate as negative to allow i128::MIN
        let mut val: i128 = 0;
        let mut saw = false;
        for b in digits.as_bytes() {
            match *b {
                b'_' => continue,
                b'0'..=b'9' => {
                    let d = (b - b'0') as i128;
                    val = val.checked_mul(10)?;
                    val = val.checked_sub(d)?;
                    saw = true;
                }
                _ => return None,
            }
        }
        if saw { Some(val) } else { None }
    } else {
        let mut val: i128 = 0;
        let mut saw = false;
        for b in digits.as_bytes() {
            match *b {
                b'_' => continue,
                b'0'..=b'9' => {
                    let d = (b - b'0') as i128;
                    val = val.checked_mul(10)?;
                    val = val.checked_add(d)?;
                    saw = true;
                }
                _ => return None,
            }
        }
        if saw { Some(val) } else { None }
    }
}

pub(crate) fn parse_int_signed<T>(
    s: String,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<i128>,
{
    let t = s.trim();
    let (neg, rest) = match t.strip_prefix('+') {
        Some(r) => (false, r),
        None => match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t),
        },
    };

    // Detect base
    let (radix, digits) = if let Some(r) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16u32, r)
    } else if let Some(r) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
        (8u32, r)
    } else if let Some(r) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
        (2u32, r)
    } else if legacy_octal && rest.starts_with("00") {
        (8u32, &rest[2..])
    } else {
        (10u32, rest)
    };

    if radix == 10 {
        let val_i128 = parse_decimal_signed_i128(digits, neg)
            .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
        return T::try_from(val_i128)
            .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location));
    }

    let mag = parse_digits_u128(digits, radix)
        .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
    let val_i128: i128 = if neg {
        let mag_i128: i128 = mag.try_into().map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))?;
        mag_i128.checked_neg().ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?
    } else {
        mag.try_into().map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))?
    };
    T::try_from(val_i128).map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))
}

pub(crate) fn parse_int_unsigned<T>(
    s: String,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<u128>,
{
    let t = s.trim();
    if t.starts_with('-') {
        return Err(Error::msg(format!("invalid {ty}")).with_location(location));
    }
    let rest = t.strip_prefix('+').unwrap_or(t);

    let (radix, digits) = if let Some(r) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16u32, r)
    } else if let Some(r) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
        (8u32, r)
    } else if let Some(r) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
        (2u32, r)
    } else if legacy_octal && rest.starts_with("00") {
        (8u32, &rest[2..])
    } else {
        (10u32, rest)
    };

    if radix == 10 {
        let val_u128 = parse_decimal_unsigned_u128(digits)
            .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
        return T::try_from(val_u128)
            .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location));
    }

    let mag = parse_digits_u128(digits, radix)
        .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
    T::try_from(mag).map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))
}

pub(crate) fn parse_yaml12_f64(s: &str, location: Location) -> Result<f64, Error> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        ".nan" | "+.nan" | "-.nan" => Ok(f64::NAN),
        ".inf" | "+.inf" => Ok(f64::INFINITY),
        "-.inf" => Ok(f64::NEG_INFINITY),
        _ => t
            .parse::<f64>()
            .map_err(|_| Error::msg("invalid floating point value").with_location(location)),
    }
}

pub(crate) fn parse_yaml12_f32(s: &str, location: Location) -> Result<f32, Error> {
    let v = parse_yaml12_f64(s, location)?;
    Ok(v as f32)
}
