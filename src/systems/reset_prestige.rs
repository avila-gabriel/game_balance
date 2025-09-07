use crate::mechanics::control;
use crate::systems::sdk::{balance_with_hooks, Hook, NominalTargets, Outcome};

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub reward_mult: f64,
    pub decay: f64,
    pub req_score: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub session_goal_minutes: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Targets {
    pub cycle_minutes: f64,
    pub reward_growth: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub rmin: f64,
    pub rmax: f64,
    pub dmin: f64,
    pub dmax: f64,
    pub qmin: f64,
    pub qmax: f64,
}
impl Bounds {
    pub fn soft() -> Self {
        Self { rmin: 1.0, rmax: 1e6, dmin: 0.0, dmax: 0.5, qmin: 1.0, qmax: 1e12 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Gains {
    pub k_r: f64,
    pub k_d: f64,
    pub k_q: f64,
}
impl Default for Gains {
    fn default() -> Self { Self { k_r: 0.6, k_d: 0.4, k_q: 0.6 } }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Obs {
    pub cycle_mins: f64,
    pub reward_rate: f64,
}

pub trait Mechanic: Hook<Params, Env, Targets, Obs> {}
impl<T: Hook<Params, Env, Targets, Obs>> Mechanic for T {}

pub fn balance_ext(
    theta0: Params,
    env: Env,
    tgt: Targets,
    b: Bounds,
    g: Gains,
    mechs: Vec<Box<dyn Mechanic>>,
    max_iters: usize,
    ref_income: f64,
) -> Outcome<Params, Obs> {
    balance_with_hooks(
        theta0,
        env,
        tgt,
        b,
        g,
        mechs
            .into_iter()
            .map(|m| m as Box<dyn Hook<_, _, _, _>>)
            .collect(),
        max_iters,
        // simulate: time to reach req_score given income with decay; reward rate
        move |th, _env, _tgt, _mechs| {
            let eff = ref_income / (1.0 + th.decay * 10.0);
            let cycle_mins = (th.req_score / eff.max(1e-6)).clamp(0.1, 1e6);
            let reward_rate = th.reward_mult / cycle_mins.max(1e-6);
            Obs { cycle_mins, reward_rate }
        },
        // nominal targets
        |th, _env, tgt, _o| {
            let reward_target = tgt.reward_growth / tgt.cycle_minutes.max(1e-6);
            NominalTargets { x: tgt.cycle_minutes, y: reward_target, z: th.decay }
        },
        // step
        |th, b, g, nom, _adj| {
            let req_target = nom.x;          // cycle target (minutes)
            let rew_rate_target = nom.y;     // desired reward/min

            let reward_mult_t = rew_rate_target * req_target;
            let decay_t = th.decay;          // leave as-is unless you want pacing tweak
            let req_score_t = th.req_score;  // idem

            let r = control::approach(th.reward_mult, reward_mult_t.clamp(b.rmin, b.rmax), g.k_r, b.rmin, b.rmax);
            let d = control::approach(th.decay,       decay_t.clamp(b.dmin, b.dmax),       g.k_d, b.dmin, b.dmax);
            let q = control::approach(th.req_score,   req_score_t.clamp(b.qmin, b.qmax),   g.k_q, b.qmin, b.qmax);
            Params { reward_mult: r, decay: d, req_score: q }
        },
        // converge if cycle within ±5%
        |o, tgt| (o.cycle_mins - tgt.cycle_minutes).abs() <= 0.05 * tgt.cycle_minutes.max(1.0),
    )
}
