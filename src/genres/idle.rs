// src/genres/idle.rs
#![cfg(feature = "genre-idle")]

//! Idle genre orchestrator.
//!
//! Coordinates neutral systems:
//! - production_spend         → reference income / utilization / TTU feel
//! - upgrade_cost_curve       → per-level TTU pacing
//! - reset_prestige           → cycle length & meta multiplier
//! - offline_accumulation     → AFK retain ratio
//!
//! You can inject *core* mechanics (e.g., draft-picked effects) via
//! [`IdleGenreHooks::core_mechs`]. These are passed into the `production_spend`
//! system on the first outer iteration. Subsequent iterations run without
//! consuming them again, avoiding the need for `Clone` on trait objects.

use crate::genres::sdk::{run_with_outer_iters, Signals};
use crate::systems::sdk::Outcome;
use crate::systems::{
    offline_accumulation as off,
    production_spend as ps,
    reset_prestige as pr,
    upgrade_cost_curve as ucc,
};

#[derive(Clone, Copy, Debug)]
pub struct IdleGenreTargets {
    // production_spend targets
    pub ttu_target_secs: f64,
    pub util_target: f64,
    pub growth_target: f64,

    // upgrade_cost_curve targets
    pub ttu_band_per_level: (f64, f64),
    pub ttu_slope_pref: f64, // e.g., 1.10 ≈ +10% TTU per level

    // prestige targets
    pub prestige_cycle_minutes: f64,
    pub prestige_growth: f64, // net growth per cycle (mult)

    // offline targets
    pub offline_retain_ratio: f64,
    pub typical_afk_minutes: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct IdleGenreConfig {
    pub max_iters_per_system: usize,
    pub outer_iters: usize,
}
impl Default for IdleGenreConfig {
    fn default() -> Self {
        Self { max_iters_per_system: 120_000, outer_iters: 2 }
    }
}

/// Hooks you can inject into the orchestrator.
/// Currently only core (production_spend) accepts mechanics.
#[derive(Default)]
pub struct IdleGenreHooks {
    pub core_mechs: Vec<Box<dyn crate::systems::production_spend::Mechanic>>,
}

#[derive(Clone, Debug)]
pub struct IdleGenreOutcome {
    pub core:     Outcome<ps::Params,  ps::Obs>,
    pub curve:    Outcome<ucc::Params, ucc::Obs>,
    pub prestige: Outcome<pr::Params,  pr::Obs>,
    pub offline:  Outcome<off::Params, off::Obs>,
}

pub fn balance_idle_genre(
    core_env: ps::Env,
    curve_env: ucc::Env,
    prestige_env: pr::Env,
    _offline_env_hint: (), // symmetry placeholder
    tgt: IdleGenreTargets,
    cfg: IdleGenreConfig,
    hooks: IdleGenreHooks,
) -> IdleGenreOutcome {
    // Seeds (could also be provided by caller)
    let mut core_theta     = ps::Params  { gen_per_sec: 10.0, spend_rate: 10.0, multiplier: 1.0 };
    let mut curve_theta    = ucc::Params { base: 10.0, growth: 1.15, track_mult: 1.0 };
    let mut prestige_theta = pr::Params  { reward_mult: 1.0, decay: 0.02, req_score: 1_000.0 };
    let mut offline_theta  = off::Params { cap_minutes: 12.0 * 60.0, decay: 0.02, efficiency: 0.6 };

    // Last outcomes we’ll return
    let (mut last_core, mut last_curve, mut last_prestige, mut last_offline) =
        (None, None, None, None);

    // We consume core_mechs on the first outer-iter; then run without them.
    // This avoids requiring Clone on Box<dyn Mechanic>.
    let mut core_mechs_once: Option<Vec<Box<dyn ps::Mechanic>>> = Some(hooks.core_mechs);

    // One outer-loop step: run all systems once and update `Signals`.
    let step = |signals_in: Signals| {
        // 1) Core production/spend — defines ref_income for the pass.
        let mechs_for_this_pass = core_mechs_once.take().unwrap_or_default();
        let core_out = ps::balance_ext(
            core_theta,
            core_env,
            ps::Targets {
                ttu_target: tgt.ttu_target_secs,
                util_target: tgt.util_target,
                growth_target: tgt.growth_target,
            },
            ps::Bounds::soft_defaults(),
            ps::Gains::default(),
            mechs_for_this_pass,
            cfg.max_iters_per_system,
        );
        core_theta = core_out.theta;
        last_core = Some(core_out.clone());

        // The *new* reference income from the core system:
        let ref_income_cur = (core_out.theta.gen_per_sec * core_out.theta.multiplier).max(0.0);

        // Choose which ref_income to use downstream:
        // - First pass: use the freshly measured value
        // - Later passes: you could smooth across passes via incoming signal; we keep it simple.
        let ref_income_for_downstream = if signals_in.ref_income > 0.0 {
            signals_in.ref_income
        } else {
            ref_income_cur
        };

        // 2) Upgrade cost curve — consumes ref_income signal.
        let curve_out = ucc::balance_ext(
            curve_theta,
            curve_env,
            ucc::Targets { ttu_band: tgt.ttu_band_per_level, slope_pref: tgt.ttu_slope_pref },
            ucc::Bounds::soft(),
            ucc::Gains::default(),
            Vec::<Box<dyn ucc::Mechanic>>::new(),
            cfg.max_iters_per_system,
            ref_income_for_downstream,
        );
        curve_theta = curve_out.theta;
        last_curve = Some(curve_out.clone());

        // 3) Prestige — consumes ref_income signal.
        let prestige_out = pr::balance_ext(
            prestige_theta,
            prestige_env,
            pr::Targets { cycle_minutes: tgt.prestige_cycle_minutes, reward_growth: tgt.prestige_growth },
            pr::Bounds::soft(),
            pr::Gains::default(),
            Vec::<Box<dyn pr::Mechanic>>::new(),
            cfg.max_iters_per_system,
            ref_income_for_downstream,
        );
        prestige_theta = prestige_out.theta;
        last_prestige = Some(prestige_out.clone());

        // 4) Offline — independent in this simple model.
        let offline_out = off::balance_ext(
            offline_theta,
            off::Env { typical_afk_minutes: tgt.typical_afk_minutes },
            off::Targets { retain_ratio: tgt.offline_retain_ratio },
            off::Bounds::soft(),
            off::Gains::default(),
            Vec::<Box<dyn off::Mechanic>>::new(),
            cfg.max_iters_per_system,
        );
        offline_theta = offline_out.theta;
        last_offline = Some(offline_out.clone());

        // Signals OUT for the next outer pass (expose the fresh core value).
        let signals_out = Signals { ref_income: ref_income_cur };

        // Return some Outcome (SDK runner wants one). Core is representative.
        (signals_out, core_out)
    };

    // Run outer iterations, threading Signals between passes.
    let (_final_signals, _outs) = run_with_outer_iters(Signals::default(), cfg.outer_iters, step);

    IdleGenreOutcome {
        core:     last_core.unwrap(),
        curve:    last_curve.unwrap(),
        prestige: last_prestige.unwrap(),
        offline:  last_offline.unwrap(),
    }
}
