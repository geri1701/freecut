//! Conversion between user-facing project units and integer geometry coordinates.
//!
//! Freecut keeps all geometry as `u32` ticks. The fixed scale is fine enough to represent
//! thousandths of a millimeter and common binary inch fractions exactly while keeping the
//! optimizer entirely integer-based.

use std::fmt;

use crate::domain::Unit;

pub const TICKS_PER_MILLIMETER: u32 = 64_000;
const TICKS_PER_INCH: u32 = 1_625_600;
const TICKS_PER_FOOT: u32 = 19_507_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseLengthError {
    Empty,
    Invalid,
    ZeroDenominator,
    Overflow,
}

impl fmt::Display for ParseLengthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("value is empty"),
            Self::Invalid => formatter.write_str("expected a decimal or fraction"),
            Self::ZeroDenominator => {
                formatter.write_str("fraction denominator must be greater than zero")
            }
            Self::Overflow => formatter.write_str("value is too large"),
        }
    }
}

impl std::error::Error for ParseLengthError {}

#[must_use]
pub const fn ticks_per_unit(unit: Unit) -> u32 {
    match unit {
        Unit::Millimeter => TICKS_PER_MILLIMETER,
        Unit::Inch => TICKS_PER_INCH,
        Unit::Foot => TICKS_PER_FOOT,
    }
}

#[must_use]
pub fn length_from_whole_units(value: u32, unit: Unit) -> Option<u32> {
    value.checked_mul(ticks_per_unit(unit))
}

#[must_use]
pub fn length_as_f64(value: u32, unit: Unit) -> f64 {
    f64::from(value) / f64::from(ticks_per_unit(unit))
}

#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the finite, nonnegative value is rounded and range-checked before conversion"
)]
pub fn length_from_f64(value: f64, unit: Unit) -> Option<u32> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }

    let scaled = (value * f64::from(ticks_per_unit(unit))).round();
    if scaled > f64::from(u32::MAX) {
        return None;
    }

    Some(scaled as u32)
}

#[must_use]
pub fn format_length(value: u32, unit: Unit) -> String {
    let maximum_precision = match unit {
        Unit::Millimeter => 6,
        Unit::Inch => 7,
        Unit::Foot => 8,
    };

    for precision in 0..=maximum_precision {
        let formatted = trimmed_decimal(value, unit, precision);
        if parse_length(&formatted, unit) == Ok(value) {
            return formatted;
        }
    }

    trimmed_decimal(value, unit, maximum_precision)
}

fn trimmed_decimal(value: u32, unit: Unit, precision: usize) -> String {
    let formatted = format!("{:.*}", precision, length_as_f64(value, unit));
    if formatted.contains('.') {
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        formatted
    }
}

#[allow(clippy::missing_errors_doc)]
pub fn parse_length(source: &str, unit: Unit) -> Result<u32, ParseLengthError> {
    let (numerator, denominator) = parse_nonnegative_rational(source)?;
    let scaled = numerator
        .checked_mul(u128::from(ticks_per_unit(unit)))
        .ok_or(ParseLengthError::Overflow)?;
    let rounded = scaled
        .checked_add(denominator / 2)
        .ok_or(ParseLengthError::Overflow)?
        / denominator;

    u32::try_from(rounded).map_err(|_error| ParseLengthError::Overflow)
}

fn parse_nonnegative_rational(source: &str) -> Result<(u128, u128), ParseLengthError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(ParseLengthError::Empty);
    }

    let parts = source.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        [value] if value.contains('/') => parse_fraction(value),
        [value] => parse_decimal(value),
        [whole, fraction] => {
            let whole = parse_integer(whole)?;
            let (fraction_numerator, denominator) = parse_fraction(fraction)?;
            let numerator = whole
                .checked_mul(denominator)
                .and_then(|value| value.checked_add(fraction_numerator))
                .ok_or(ParseLengthError::Overflow)?;
            Ok((numerator, denominator))
        }
        _ => Err(ParseLengthError::Invalid),
    }
}

fn parse_fraction(source: &str) -> Result<(u128, u128), ParseLengthError> {
    let (numerator, denominator) = source.split_once('/').ok_or(ParseLengthError::Invalid)?;
    if denominator.contains('/') {
        return Err(ParseLengthError::Invalid);
    }

    let numerator = parse_integer(numerator)?;
    let denominator = parse_integer(denominator)?;
    if denominator == 0 {
        return Err(ParseLengthError::ZeroDenominator);
    }

    Ok((numerator, denominator))
}

fn parse_decimal(source: &str) -> Result<(u128, u128), ParseLengthError> {
    if source.starts_with('-') || source.starts_with('+') || source.contains(['e', 'E']) {
        return Err(ParseLengthError::Invalid);
    }
    if source.contains('.') && source.contains(',') {
        return Err(ParseLengthError::Invalid);
    }

    let normalized = source.replace(',', ".");
    let Some((whole, fraction)) = normalized.split_once('.') else {
        return Ok((parse_integer(&normalized)?, 1));
    };
    if fraction.contains('.') || (whole.is_empty() && fraction.is_empty()) {
        return Err(ParseLengthError::Invalid);
    }

    let whole = if whole.is_empty() {
        0
    } else {
        parse_integer(whole)?
    };
    if fraction.is_empty() {
        return Ok((whole, 1));
    }
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ParseLengthError::Invalid);
    }

    let denominator = 10_u128
        .checked_pow(u32::try_from(fraction.len()).map_err(|_error| ParseLengthError::Overflow)?)
        .ok_or(ParseLengthError::Overflow)?;
    let fraction = parse_integer(fraction)?;
    let numerator = whole
        .checked_mul(denominator)
        .and_then(|value| value.checked_add(fraction))
        .ok_or(ParseLengthError::Overflow)?;

    Ok((numerator, denominator))
}

fn parse_integer(source: &str) -> Result<u128, ParseLengthError> {
    if source.is_empty() || !source.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ParseLengthError::Invalid);
    }

    source
        .parse::<u128>()
        .map_err(|_error| ParseLengthError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_and_fractional_lengths_exactly() {
        assert_eq!(parse_length("3.175", Unit::Millimeter), Ok(203_200));
        assert_eq!(parse_length("0.125", Unit::Inch), Ok(203_200));
        assert_eq!(parse_length("1/8", Unit::Inch), Ok(203_200));
        assert_eq!(parse_length("12 1/8", Unit::Inch), Ok(19_710_400));
        assert_eq!(parse_length("3,175", Unit::Millimeter), Ok(203_200));
    }

    #[test]
    fn rejects_invalid_fractions_and_overflow() {
        assert_eq!(
            parse_length("1/0", Unit::Inch),
            Err(ParseLengthError::ZeroDenominator)
        );
        assert_eq!(
            parse_length("-1", Unit::Millimeter),
            Err(ParseLengthError::Invalid)
        );
        assert_eq!(
            parse_length("1/2/3", Unit::Inch),
            Err(ParseLengthError::Invalid)
        );
        assert_eq!(
            parse_length("999999", Unit::Foot),
            Err(ParseLengthError::Overflow)
        );
    }

    #[test]
    fn changing_display_units_preserves_the_physical_length() {
        let one_inch = parse_length("1", Unit::Inch).expect("parse inch");

        assert_eq!(format_length(one_inch, Unit::Millimeter), "25.4");
        assert_eq!(format_length(one_inch, Unit::Foot), "0.08333333");
    }

    #[test]
    fn formatting_is_compact_and_roundtrips_geometry_ticks() {
        for unit in [Unit::Millimeter, Unit::Inch, Unit::Foot] {
            let ticks = ticks_per_unit(unit);
            for value in [0, 1, ticks - 1, ticks, ticks + 1, 203_200, u32::MAX] {
                let formatted = format_length(value, unit);
                assert_eq!(parse_length(&formatted, unit), Ok(value), "{formatted}");
            }
        }

        assert_eq!(format_length(203_200, Unit::Millimeter), "3.175");
        assert_eq!(format_length(203_200, Unit::Inch), "0.125");
        assert_eq!(
            format_length(
                parse_length("0.001", Unit::Inch).expect("parse"),
                Unit::Inch
            ),
            "0.001"
        );
    }
}
