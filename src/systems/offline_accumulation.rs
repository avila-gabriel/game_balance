use crate::mechanics::control;
use crate::systems::sdk::{Hook, NominalTargets, Outcome, balance_with_hooks};

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub cap_minutes: f64,
    pub decay: f64,
    pub efficiency: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub typical_afk_minutes: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Targets {
    pub retain_ratio: f64, /* target offline/online income ratio for typical AFK */
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub cmin: f64,
    cmax: f64,
    dmin: f64,
    dmax: f64,
    emin: f64,
    emax: f64,
}
impl Bounds {
    pub fn soft() -> Self {
        Self {
            cmin: 10.0,
            cmax: 72.0 * 60.0,
            dmin: 0.0,
            dmax: 0.1,
            emin: 0.0,
            emax: 1.0,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Gains {
    pub k_c: f64,
    k_d: f64,
    k_e: f64,
}
impl Default for Gains {
    fn default() -> Self {
        Self {
            k_c: 0.6,
            k_d: 0.4,
            k_e: 0.6,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Obs {
    pub retain: f64,
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
        move |th, env, _tgt, _mechs| {
            let t = env.typical_afk_minutes;
            let effective = th.efficiency * (1.0 - th.decay).powf(t / th.cap_minutes.max(1.0));
            Obs {
                retain: effective.clamp(0.0, 1.0),
            }
        },
        |th, _env, tgt, _o| NominalTargets {
            x: tgt.retain_ratio,
            y: th.cap_minutes,
            z: th.decay,
        },
        |th, b, g, nom, _adj| {
            let efficiency_t = nom.x;
            let cap_t = th.cap_minutes;
            let decay_t = th.decay;

            let cap_minutes = control::approach(
                th.cap_minutes,
                cap_t.clamp(b.cmin, b.cmax),
                g.k_c,
                b.cmin,
                b.cmax,
            );
            let decay = control::approach(
                th.decay,
                decay_t.clamp(b.dmin, b.dmax),
                g.k_d,
                b.dmin,
                b.dmax,
            );
            let efficiency = control::approach(
                th.efficiency,
                efficiency_t.clamp(b.emin, b.emax),
                g.k_e,
                b.emin,
                b.emax,
            );

            Params {
                cap_minutes,
                decay,
                efficiency,
            }
        },
        |o, tgt| (o.retain - tgt.retain_ratio).abs() <= 0.02,
    )
}
