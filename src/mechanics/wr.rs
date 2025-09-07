/// Win-rate mechanics (linear, tanh, analytic inversion).

/// Linear WR from attack-vs-defend with baseline 0.5.
#[inline]
pub fn linear(eff_actions: f64, defend_rate: f64) -> f64 {
    0.5 + eff_actions * ((1.0 - defend_rate) - 0.5)
}

/// Diminishing returns via tanh: WR = 0.5 + β * tanh(pressure).
#[inline]
pub fn tanh(eff_actions: f64, defend_rate: f64, alpha: f64, beta: f64) -> f64 {
    let pressure = alpha * eff_actions * (1.0 - defend_rate);
    0.5 + beta * pressure.tanh()
}

/// Invert tanh WR to required effective action rate for a target WR.
#[inline]
pub fn eff_from_target(wr_target: f64, defend_rate: f64, alpha: f64, beta: f64) -> f64 {
    fn atanh_safe(x: f64) -> f64 {
        let x = x.clamp(-0.999_999_9, 0.999_999_9);
        0.5 * ((1.0 + x) / (1.0 - x)).ln()
    }
    let lift = (wr_target - 0.5) / beta;
    let eff_raw = atanh_safe(lift) / (alpha * (1.0 - defend_rate));
    eff_raw.clamp(0.0, 1.0)
}

/// Explicit pressure form: α * eff * (1 - defend_rate) * mults.
#[inline]
pub fn pressure(alpha: f64, eff: f64, one_minus_defend: f64, mult: f64) -> f64 {
    alpha * eff * one_minus_defend * mult
}

/// From pressure  WR.
#[inline]
pub fn from_pressure(pressure: f64, beta: f64) -> f64 {
    0.5 + beta * pressure.tanh()
}
