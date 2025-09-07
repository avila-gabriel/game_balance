/// Control mechanics: proportional updates.

/// Proportional approach: x' = clamp(x + k * (target - x)).
#[inline]
pub fn approach(x: f64, target: f64, k: f64, lo: f64, hi: f64) -> f64 {
    (x + k * (target - x)).clamp(lo, hi)
}

/// Proportional against signed error: x' = clamp(x - k * error).
#[inline]
pub fn p_against_error(x: f64, error: f64, k: f64, lo: f64, hi: f64) -> f64 {
    (x - k * error).clamp(lo, hi)
}
