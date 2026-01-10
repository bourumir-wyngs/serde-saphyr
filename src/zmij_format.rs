/// Format as float string, make changes to be sure valid YAML float (zmij may render 4e-6 and not 4.0e-6)

use std::fmt::Write;
use zmij::Float;
use num_traits::float::FloatCore;
use crate::ser;

/// Format as float string, make changes to be sure valid YAML float
pub(crate) fn push_float_string<F: Float + FloatCore>(target: & mut String, f: F) -> ser::Result<()> {
    if f.is_nan() {
        target.push_str(".nan");
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            target.push_str(".inf");
        } else {
            target.push_str("-.inf");
        }
    } else {
        let mut buf = zmij::Buffer::new();
        // Branches .is_nan and .is_infinite are already covered above
        let s = buf.format_finite(f);
        target.reserve(s.len() + 3);
        // YAML 1.1 float requires:
        // - a decimal point in the mantissa (avoid integers being parsed as int)
        // - a sign (+ or -) in the exponent (when exponent is present)
        if let Some(exp_pos) = s.find('e').or_else(|| s.find('E')) {
            // 1) Write mantissa, ensuring it has a decimal point.
            if !s[..exp_pos].as_bytes().contains(&b'.') {
                // "4e-6" -> "4.0e-6"
                target.push_str(&s[..exp_pos]);
                target.push_str(".0");
            } else {
                target.push_str(&s[..exp_pos]);
            }

            // 2) Write exponent marker.
            target.push_str(&s[exp_pos..=exp_pos]);

            // 3) Ensure exponent sign.
            if let Some(after_e) = s.as_bytes().get(exp_pos + 1) {
                if after_e != &b'+' && after_e != &b'-' {
                    // "1e6" -> "1e+6"
                    target.push('+');
                }
            }
            target.push_str(&s[exp_pos + 1..]);
        } else if !s.as_bytes().contains(&b'.') {
            // No decimal and no exponent: append .0
            target.push_str(s);
            target.push_str(".0");
        } else {
            target.push_str(s);
        }
    }
    Ok(())
}

/// Format as float string, make changes to be sure valid YAML float
pub(crate) fn write_float_string<F: Float + FloatCore, W: Write>(target: &mut W, f: F) -> ser::Result<()> {
    if f.is_nan() {
        target.write_str(".nan")?;
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            target.write_str(".inf")?;
        } else {
            target.write_str("-.inf")?;
        }
    } else {
        let mut buf = zmij::Buffer::new();
        // Branches .is_nan and .is_infinite are already covered above
        let s = buf.format_finite(f);
        // YAML 1.1 float requires:
        // - a decimal point in the mantissa (avoid integers being parsed as int)
        // - a sign (+ or -) in the exponent (when exponent is present)
        if let Some(exp_pos) = s.find('e').or_else(|| s.find('E')) {
            // 1) Write mantissa, ensuring it has a decimal point.
            if !s[..exp_pos].as_bytes().contains(&b'.') {
                // "4e-6" -> "4.0e-6"
                target.write_str(&s[..exp_pos])?;
                target.write_str(".0")?;
            } else {
                target.write_str(&s[..exp_pos])?;
            }

            // 2) Write exponent marker.
            target.write_str(&s[exp_pos..=exp_pos])?;

            // 3) Ensure exponent sign.
            if let Some(after_e) = s.as_bytes().get(exp_pos + 1) {
                if after_e != &b'+' && after_e != &b'-' {
                    // "1e6" -> "1e+6"
                    target.write_char('+')?;
                }
            }
            target.write_str(&s[exp_pos + 1..])?;
        } else if !s.as_bytes().contains(&b'.') {
            // No decimal and no exponent: append .0
            target.write_str(s)?;
            target.write_str(".0")?;
        } else {
            target.write_str(s)?;
        }
    }
    Ok(())
}