/// Stochastic mechanics: RNG helpers and crit/jitter multipliers.
/// Note: uses `bevy_prng::WyRand` with `Rc<RefCell<>>` so callers
/// can keep closures `Fn` while mutating RNG state.
use bevy_prng::WyRand;
use rand_core::RngCore;
use std::cell::RefCell;

/// Gaussian(0,1) via BoxMuller using WyRand.
#[inline]
pub fn gaussian01(rng: &RefCell<WyRand>) -> f64 {
    let mut r = rng.borrow_mut();
    let u1 = ((r.next_u64() >> 11) as f64) / ((1u64 << 53) as f64);
    let u2 = ((r.next_u64() >> 11) as f64) / ((1u64 << 53) as f64);
    drop(r);
    let r = (-2.0 * u1.ln()).sqrt();
    let t = 2.0 * std::f64::consts::PI * u2;
    r * t.cos()
}

/// Bernoulli(p) with WyRand.
#[inline]
pub fn bernoulli(rng: &RefCell<WyRand>, p: f64) -> bool {
    let mut r = rng.borrow_mut();
    let u = ((r.next_u64() >> 11) as f64) / ((1u64 << 53) as f64);
    drop(r);
    u < p.clamp(0.0, 1.0)
}

/// Crit multiplier factor (1 or mult).
#[inline]
pub fn crit_factor(rng: &RefCell<WyRand>, chance: f64, mult: f64) -> f64 {
    if bernoulli(rng, chance) { mult } else { 1.0 }
}

/// Multiplicative damage jitter: max(0, 1 + N(0,1)*jitter).
#[inline]
pub fn dmg_noise(rng: &RefCell<WyRand>, jitter: f64) -> f64 {
    (1.0 + gaussian01(rng) * jitter).max(0.0)
}
