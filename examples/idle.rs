// examples/idle.rs
// Run with:
//   cargo run --example idle --features "genre-idle system-production_spend system-upgrade_cost_curve system-reset_prestige system-offline_accumulation"

use game_balance::genres::idle::*;
use game_balance::systems::{production_spend as ps, upgrade_cost_curve as ucc, reset_prestige as pr};

fn main() {
    // Environments per system (pick your own numbers)
    let core_env = ps::Env {
        upgrade_cost_base: 10.0,
        upgrade_cost_growth: 1.15,
        gain_per_level: 0.05,
        leak: 0.02,
        storage_cap: 100_000.0,
    };

    let curve_env = ucc::Env {
        levels: 10,
        gain_per_level: 0.05,
    };

    let prestige_env = pr::Env {
        session_goal_minutes: 20.0,
    };

    // Genre targets
    let tgt = IdleGenreTargets {
        // production_spend
        ttu_target_secs: 30.0,
        util_target: 0.90,
        growth_target: 5.0,

        // upgrade_cost_curve
        ttu_band_per_level: (7.5, 9.5),
        ttu_slope_pref: 1.15,

        // prestige
        prestige_cycle_minutes: 20.0,
        prestige_growth: 10.0,

        // offline
        offline_retain_ratio: 0.70,
        typical_afk_minutes: 180.0,
    };

    let cfg = IdleGenreConfig {
        max_iters_per_system: 120_000,
        outer_iters: 2,
    };

    let out = balance_idle_genre(core_env, curve_env, prestige_env, (), tgt, cfg);

    println!("== Idle Genre Outcome ==");
    println!("Core   θ -> {:?}", out.core.theta);
    println!("Core   π -> {:?}", out.core.obs);
    println!("Curve  θ -> {:?}", out.curve.theta);
    println!("Curve  π -> {:?}", out.curve.obs);
    println!("Prest  θ -> {:?}", out.prestige.theta);
    println!("Prest  π -> {:?}", out.prestige.obs);
    println!("Offline θ -> {:?}", out.offline.theta);
    println!("Offline π -> {:?}", out.offline.obs);
}
