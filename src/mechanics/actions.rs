/// Mechanics for action rates (attack/play/etc).
/// Economy and energy caps + effective rate combination.

/// Economy-only cap: actions  prod / cost.
#[inline]
pub fn econ_cap(prod: f64, cost: f64) -> f64 {
    (prod / cost).clamp(0.0, 1.0)
}

/// Combine desired rate with two caps (e.g., econ & energy).
#[inline]
pub fn effective(desired: f64, cap_a: f64, cap_b: f64) -> f64 {
    desired.min(cap_a).min(cap_b).clamp(0.0, 1.0)
}
