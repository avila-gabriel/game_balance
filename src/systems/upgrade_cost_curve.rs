use crate::mechanics::control;
use crate::systems::sdk::{balance_with_hooks, Hook, NominalTargets, Outcome};

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub base: f64,       // C0
    pub growth: f64,     // g > 1
    pub track_mult: f64, // per-track scaling
}

#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub levels: u32,        // upgrades in this “chapter”
    pub gain_per_level: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Targets {
    pub ttu_band: (f64, f64), // desired TTU band per level
    pub slope_pref: f64,      // TTU_{L+1}/TTU_L preference, e.g. 1.10
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub base_min: f64,  pub base_max: f64,
    pub growth_min: f64, pub growth_max: f64,
    pub mult_min: f64,  pub mult_max: f64,
}
impl Bounds {
    pub fn soft() -> Self {
        Self {
            base_min: 1.0, base_max: 1e9,
            growth_min: 1.01, growth_max: 2.5,
            mult_min: 0.1, mult_max: 100.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Gains { pub k_base: f64, pub k_growth: f64, pub k_mult: f64 }
impl Default for Gains { fn default() -> Self { Self { k_base: 0.6, k_growth: 0.4, k_mult: 0.5 } } }

#[derive(Clone, Copy, Debug, Default)]
pub struct Obs {
    pub ttu_mean: f64,  // mean TTU over levels under a reference income
    pub ttu_slope: f64, // average TTU_{L+1}/TTU_L
}

pub trait Mechanic: Hook<Params, Env, Targets, Obs> {}
impl<T: Hook<Params, Env, Targets, Obs>> Mechanic for T {}

pub fn balance_ext(
    theta0: Params,
    env: Env,
    tgt: Targets,
    bnd: Bounds,
    g: Gains,
    mechs: Vec<Box<dyn Mechanic>>,
    max_iters: usize,
    ref_income: f64,
) -> Outcome<Params, Obs> {
    balance_with_hooks(
        theta0,
        env,
        tgt,
        bnd,
        g,
        mechs.into_iter().map(|m| m as Box<dyn Hook<_, _, _, _>>).collect(),
        max_iters,
        // simulate: approximate TTU given cost_k and ref income
        move |th, env, _tgt, _mechs| {
            let n = env.levels as usize;
            let mut sum = 0.0;
            let mut slope_acc = 0.0;
            let mut prev_ttu: Option<f64> = None;

            for l in 0..n {
                let lvl = l as f64;
                let cost = th.base * th.growth.powf(lvl) * th.track_mult;
                // Proxy: assume ~90% utilization → 10% savings
                let save_rate = (1.0_f64 - 0.9_f64).max(0.1) * ref_income;
                let ttu = (cost / save_rate.max(1e-9)).clamp(0.0, 86_400.0);

                sum += ttu;
                if let Some(p) = prev_ttu {
                    slope_acc += (ttu / p).clamp(0.1, 10.0);
                }
                prev_ttu = Some(ttu);
            }

            let ttu_mean = sum / (n.max(1) as f64);
            let ttu_slope = if n > 1 { slope_acc / ((n - 1) as f64) } else { 1.0 };
            Obs { ttu_mean, ttu_slope }
        },
        // nominal: target mean & slope from band
        |th, _env, tgt, _o| {
            let target_mean  = 0.5 * (tgt.ttu_band.0 + tgt.ttu_band.1);
            let target_slope = tgt.slope_pref.max(1.0);
            NominalTargets { x: target_mean, y: target_slope, z: th.track_mult } // z unused here
        },
        // step: base→mean, growth→slope, mult as buffer
        |th, b, g, nom, _adj| {
            let desired_mean  = nom.x;
            let desired_slope = nom.y;

            // Gentle base adjustment toward desired mean (kept simple/monotone).
            let base_target   = th.base * (desired_mean / desired_mean.max(1e-9)).max(0.5);
            let growth_target = (th.growth * desired_slope / desired_slope.max(1e-9))
                .clamp(b.growth_min, b.growth_max);
            let mult_target   = th.track_mult;

            let base       = control::approach(th.base,       base_target.clamp(b.base_min, b.base_max),   g.k_base,  b.base_min,  b.base_max);
            let growth     = control::approach(th.growth,     growth_target,                               g.k_growth,b.growth_min,b.growth_max);
            let track_mult = control::approach(th.track_mult, mult_target.clamp(b.mult_min, b.mult_max),   g.k_mult,  b.mult_min,  b.mult_max);

            Params { base, growth, track_mult }
        },
        // converged: mean TTU within band & slope near target
        |o, tgt| {
            let mean_ok  = o.ttu_mean >= tgt.ttu_band.0 && o.ttu_mean <= tgt.ttu_band.1;
            let slope_ok = (o.ttu_slope - tgt.slope_pref).abs() <= 0.05;
            mean_ok && slope_ok
        },
    )
}
