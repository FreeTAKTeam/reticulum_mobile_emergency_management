#[must_use]
pub(crate) fn u64_to_i64_saturating(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[must_use]
pub(crate) fn i64_to_u64_saturating(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

#[must_use]
pub(crate) fn usize_to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[must_use]
pub(crate) fn u64_to_u32_saturating(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[must_use]
pub(crate) fn i64_to_u8_saturating(value: i64) -> u8 {
    u8::try_from(value).unwrap_or(if value.is_negative() { 0 } else { u8::MAX })
}

#[must_use]
pub(crate) fn u128_to_u64_saturating(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[must_use]
#[cfg(test)]
pub(crate) fn u128_to_u32_saturating(value: u128) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

// Float-to-integer conversion has no checked standard-library equivalent. These helpers
// deliberately normalize non-finite values and clamp into the destination range first.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub(crate) fn f64_to_u32_saturating(value: f64) -> u32 {
    finite_or_zero(value)
        .round()
        .clamp(0.0, f64::from(u32::MAX)) as u32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub(crate) fn f64_to_u8_saturating(value: f64) -> u8 {
    finite_or_zero(value).round().clamp(0.0, f64::from(u8::MAX)) as u8
}

#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub(crate) fn f64_to_i64_saturating(value: f64) -> i64 {
    finite_or_zero(value)
        .round()
        .clamp(i64::MIN as f64, i64::MAX as f64) as i64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub(crate) fn f64_to_u64_saturating(value: f64) -> u64 {
    finite_or_zero(value).round().clamp(0.0, u64::MAX as f64) as u64
}

#[allow(clippy::cast_precision_loss)]
#[must_use]
pub(crate) fn i64_to_f64_exact(value: i64) -> Option<f64> {
    const MAX_EXACT_INTEGER: i64 = 1_i64 << f64::MANTISSA_DIGITS;
    (-MAX_EXACT_INTEGER..=MAX_EXACT_INTEGER)
        .contains(&value)
        .then_some(value as f64)
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_boundaries_are_deliberately_saturated() {
        assert_eq!(u64_to_i64_saturating(u64::MAX), i64::MAX);
        assert_eq!(i64_to_u64_saturating(-1), 0);
        assert_eq!(usize_to_u32_saturating(usize::MAX), u32::MAX);
        assert_eq!(u64_to_u32_saturating(u64::MAX), u32::MAX);
        assert_eq!(i64_to_u8_saturating(-1), 0);
        assert_eq!(i64_to_u8_saturating(i64::MAX), u8::MAX);
    }

    #[test]
    fn float_boundaries_are_finite_and_clamped() {
        assert_eq!(f64_to_u32_saturating(f64::NAN), 0);
        assert_eq!(f64_to_u8_saturating(300.0), u8::MAX);
        assert_eq!(f64_to_i64_saturating(f64::INFINITY), 0);
        assert_eq!(f64_to_u64_saturating(-1.0), 0);
        assert_eq!(
            i64_to_f64_exact(9_007_199_254_740_992),
            Some(9_007_199_254_740_992.0)
        );
        assert_eq!(i64_to_f64_exact(9_007_199_254_740_993), None);
    }
}
