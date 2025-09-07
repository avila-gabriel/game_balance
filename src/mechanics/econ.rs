/// Economy mechanics: surplus and (optionally) storage.

/// Per-turn surplus: prod - upkeep - actions*cost.
#[inline]
pub fn surplus(prod: f64, upkeep: f64, actions: f64, cost: f64) -> f64 {
    prod - upkeep - actions * cost
}

/// Steady storage S* for dS/dt = surplus - leak*S, clamped to [0, cap].
#[inline]
pub fn storage_steady(surplus: f64, leak: f64, cap: f64) -> f64 {
    let s = if leak > 0.0 { surplus / leak } else { cap };
    if s.is_finite() {
        s.clamp(0.0, cap)
    } else {
        0.0
    }
}

/// Max affordable action rate given production and upkeep.
#[inline]
pub fn spend_cap(prod: f64, upkeep: f64, cost: f64) -> f64 {
    ((prod - upkeep).max(0.0) / cost).clamp(0.0, 1.0)
}
