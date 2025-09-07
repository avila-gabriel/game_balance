//! fees: extra generation multiplier based on money / production.
//! Example: mult = 1 + slope * (money / max(prod, eps)), clamped to [1, max_mult].

#[inline]
pub fn multiplier_from_money_over_prod(money: f64, prod: f64, slope: f64, max_mult: f64) -> f64 {
    let denom = prod.abs().max(1e-9);
    let raw = 1.0 + slope.max(0.0) * (money / denom);
    raw.clamp(1.0, max_mult.max(1.0))
}
