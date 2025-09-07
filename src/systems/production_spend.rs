use crate::mechanics::{actions, control};
use crate::systems::sdk::{Hook, NominalTargets, Outcome, balance_with_hooks};

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub gen_per_sec: f64,
    pub spend_rate: f64,
    pub multiplier: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub upgrade_cost_base: f64,
    pub upgrade_cost_growth: f64,
    pub gain_per_level: f64,
    pub leak: f64,
    pub storage_cap: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Targets {
    pub ttu_target: f64,
    pub util_target: f64,
    pub growth_target: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub gen_min: f64,
    pub gen_max: f64,
    pub spd_min: f64,
    pub spd_max: f64,
    pub mul_min: f64,
    pub mul_max: f64,
}
impl Bounds {
    pub fn soft_defaults() -> Self {
        Self {
            gen_min: 0.01,
            gen_max: 1e6,
            spd_min: 0.0,
            spd_max: 1e9,
            mul_min: 0.1,
            mul_max: 1e6,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Gains {
    pub k_ttu: f64,
    pub k_util: f64,
    pub k_grow: f64,
}

impl Default for Gains {
    fn default() -> Self {
        Self {
            k_ttu: 0.6,
            k_util: 0.6,
            k_grow: 0.5,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Obs {
    pub ttu: f64,
    pub util: f64,
    pub growth: f64,
    pub surplus: f64,
}

pub trait Mechanic: Hook<Params, Env, Targets, Obs> {}
impl<T: Hook<Params, Env, Targets, Obs>> Mechanic for T {}

pub fn balance_quick(env: Env, tgt: Targets) -> Outcome<Params, Obs> {
    balance_ext(
        Params {
            gen_per_sec: 10.0,
            spend_rate: 10.0,
            multiplier: 1.0,
        },
        env,
        tgt,
        Bounds::soft_defaults(),
        Gains::default(),
        Vec::new(),
        120_000,
    )
}

pub fn balance_ext(
    theta0: Params,
    env: Env,
    tgt: Targets,
    bnd: Bounds,
    gains: Gains,
    mechs: Vec<Box<dyn Mechanic>>,
    max_iters: usize,
) -> Outcome<Params, Obs> {
    balance_with_hooks(
        theta0,
        env,
        tgt,
        bnd,
        gains,
        // NOTE: accept mechanics as trait objects; no Sized bound headaches.
        mechs
            .into_iter()
            .map(|m| m as Box<dyn Hook<_, _, _, _>>)
            .collect(),
        max_iters,
        /* simulate */
        |th, env, tgt, mechs| {
            let mut income = (th.gen_per_sec * th.multiplier).max(0.0);
            for m in mechs.iter_mut() {
                income *= m.income_multiplier(income, th, env).max(0.0);
            }
            let cap = actions::econ_cap(income, 1.0);
            let spend = (th.spend_rate.min(income) * cap).clamp(0.0, income);
            let surplus = income - spend;

            let lvl = (th.multiplier / env.gain_per_level).max(0.0);
            let cost_next = env.upgrade_cost_base * env.upgrade_cost_growth.powf(lvl);

            let util = if income > 0.0 {
                (spend / income).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let save_floor: f64 = (1.0 - tgt.util_target).clamp(0.0, 1.0);
            let eff_save = (income - spend).max(income * save_floor).max(1e-9);
            let ttu = (cost_next / eff_save).clamp(0.0, 86_400.0);

            let growth = if income > 0.0 {
                th.multiplier * (1.0 + (surplus.max(0.0) / income))
            } else {
                th.multiplier
            };

            Obs {
                ttu,
                util,
                growth,
                surplus,
            }
        },
        /* nominal targets */
        |th, env, tgt, o| {
            let save_floor: f64 = (1.0 - tgt.util_target).clamp(1e-6, 1.0);
            let lvl = (th.multiplier / env.gain_per_level).max(0.0);
            let cost_next = env.upgrade_cost_base * env.upgrade_cost_growth.powf(lvl);
            let saving_star = (cost_next / tgt.ttu_target.max(1e-6)).max(0.0);
            let income_star = (saving_star / save_floor).max(1e-9);

            // x = income*, y = spend*, z = mult* (nominal)
            NominalTargets {
                x: income_star,
                y: tgt.util_target * income_star,
                z: th.multiplier * (tgt.growth_target / o.growth.max(1e-9)).clamp(0.5, 2.0),
            }
        },
        /* step */
        |th, bnd, g, nom, adj| {
            let gen_target = (nom.x / th.multiplier.max(1e-9)) * adj.a;
            let spend_target = nom.y * adj.b;
            let mult_target = nom.z * adj.c;

            let r#gen_next = control::approach(
                th.gen_per_sec,
                gen_target.clamp(bnd.gen_min, bnd.gen_max),
                g.k_ttu,
                bnd.gen_min,
                bnd.gen_max,
            );
            let spd_next = control::approach(
                th.spend_rate,
                spend_target.clamp(bnd.spd_min, bnd.spd_max),
                g.k_util,
                bnd.spd_min,
                bnd.spd_max,
            );
            let mul_next = control::approach(
                th.multiplier,
                mult_target.clamp(bnd.mul_min, bnd.mul_max),
                g.k_grow,
                bnd.mul_min,
                bnd.mul_max,
            );

            Params {
                gen_per_sec: r#gen_next,
                spend_rate: spd_next,
                multiplier: mul_next,
            }
        },
        /* converged */
        |o, tgt| {
            (o.ttu - tgt.ttu_target).abs() <= 0.02 * tgt.ttu_target.max(1.0)
                && (o.util - tgt.util_target).abs() <= 0.01
                && (o.growth - tgt.growth_target).abs() <= 0.02 * tgt.growth_target.max(1.0)
        },
    )
}
