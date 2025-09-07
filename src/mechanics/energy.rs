/// Energy mechanics: caps and utilization.

/// Energy cap: actions  energy / cost.
#[inline]
pub fn cap(energy: f64, cost: f64) -> f64 {
    (energy / cost).clamp(0.0, 1.0)
}

/// Utilization of energy budget (0..1).
#[inline]
pub fn utilization(spend: f64, energy: f64) -> f64 {
    if energy > 0.0 {
        (spend / energy).clamp(0.0, 1.0)
    } else {
        0.0
    }
}
