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

pub(crate) fn parse_num<T: std::str::FromStr>(
    s: String,
    ty: &'static str,
    location: Location,
) -> Result<T, Error> {
    s.parse()
        .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))
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
