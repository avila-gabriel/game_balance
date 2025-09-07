// Run with:
//   cargo run --example idle_draft \
//     --features "genre-idle \
//                 system-production_spend system-upgrade_cost_curve \
//                 system-reset_prestige system-offline_accumulation \
//                 system-draft_choice"

use game_balance::genres::idle::*;
use game_balance::systems::{
    production_spend as ps,
    draft_choice as draft,
    sdk::{Hook, NominalTargets, TargetAdjust},
};

// ---------- Adapter: dyn Hook -> dyn ps::Mechanic ----------
struct HookAdapter(Box<dyn Hook<ps::Params, ps::Env, ps::Targets, ps::Obs>>);

impl Hook<ps::Params, ps::Env, ps::Targets, ps::Obs> for HookAdapter {
    fn income_multiplier(&mut self, base: f64, th: &ps::Params, env: &ps::Env) -> f64 {
        self.0.income_multiplier(base, th, env)
    }
    fn on_observe(&mut self, o: &ps::Obs, th: &ps::Params, env: &ps::Env, tgt: &ps::Targets) {
        self.0.on_observe(o, th, env, tgt)
    }
    fn adjust_targets(
        &mut self,
        th: &ps::Params,
        env: &ps::Env,
        tgt: &ps::Targets,
        nom: &NominalTargets,
    ) -> TargetAdjust {
        self.0.adjust_targets(th, env, tgt, nom)
    }
}
// Blanket impl in ps turns any Hook into a Mechanic automatically.
// (ps::Mechanic: Hook<...>; impl<T: Hook<...>> Mechanic for T {})

// ---------- Example hooks ----------
struct IncomeMult { mult: f64 }
impl Hook<ps::Params, ps::Env, ps::Targets, ps::Obs> for IncomeMult {
    fn income_multiplier(&mut self, _base: f64, _th: &ps::Params, _env: &ps::Env) -> f64 {
        self.mult
    }
}

struct UtilNudge { add: f64 }
impl Hook<ps::Params, ps::Env, ps::Targets, ps::Obs> for UtilNudge {
    fn adjust_targets(
        &mut self,
        _th: &ps::Params,
        _env: &ps::Env,
        _tgt: &ps::Targets,
        _nom: &NominalTargets,
    ) -> TargetAdjust {
        TargetAdjust { a: 1.0, b: 1.0 + self.add, c: 1.0 }
    }
}

struct GrowthNudge { mult: f64 }
impl Hook<ps::Params, ps::Env, ps::Targets, ps::Obs> for GrowthNudge {
    fn adjust_targets(
        &mut self,
        _th: &ps::Params,
        _env: &ps::Env,
        _tgt: &ps::Targets,
        _nom: &NominalTargets,
    ) -> TargetAdjust {
        TargetAdjust { a: 1.0, b: 1.0, c: self.mult }
    }
}

fn main() {
    // 1) Draft pool (independent base probabilities)
    let pool: Vec<draft::EffectCard<ps::Params, ps::Env, ps::Targets, ps::Obs>> = vec![
        draft::EffectCard {
            name: "Income +10%".into(),
            tier: draft::Tier::Common,
            base_p: 0.5,
            pity: None,
            mk: Box::new(|| Box::new(IncomeMult { mult: 1.10 })),
        },
        draft::EffectCard {
            name: "Income +25%".into(),
            tier: draft::Tier::Uncommon,
            base_p: 0.3,
            pity: None,
            mk: Box::new(|| Box::new(IncomeMult { mult: 1.25 })),
        },
        draft::EffectCard {
            name: "Util +5%".into(),
            tier: draft::Tier::Rare,
            base_p: 0.2,
            pity: None,
            mk: Box::new(|| Box::new(UtilNudge { add: 0.05 })),
        },
        draft::EffectCard {
            name: "Growth ×1.5".into(),
            tier: draft::Tier::Epic,
            base_p: 0.1,
            pity: None,
            mk: Box::new(|| Box::new(GrowthNudge { mult: 1.5 })),
        },
    ];

    let cfg_draft = draft::DraftConfig {
        options_per_roll: 2,
        rerolls_per_draft: 1,
        prioritize_tier: true,
    };
    let mut draft_state = draft::DraftState::new(cfg_draft, pool.len(), 12345);

    // 2) Show offer & pick
    let offer = draft::make_offer(&pool, cfg_draft, &mut draft_state);
    println!("Draft Offer:");
    for (i, c) in offer.iter().enumerate() {
        println!("  {}. {} ({:?})", i, c.name, c.tier);
    }
    // pretend the player picks 0 (you could add input/UI here)
    let pick_idx = 0;
    let picked_hook_dyn = draft::instantiate_hook(&pool, &offer[pick_idx]);
    draft::notify_picked(&pool, &mut draft_state, &offer, pick_idx);

    // Wrap the Hook into a ps::Mechanic via the adapter
    let core_mech: Box<dyn ps::Mechanic> = Box::new(HookAdapter(picked_hook_dyn));

    // 3) Run the IDLE GENRE (not just the system) with this core hook
    // Environments
    let core_env = ps::Env {
        upgrade_cost_base: 10.0,
        upgrade_cost_growth: 1.15,
        gain_per_level: 0.05,
        leak: 0.02,
        storage_cap: 100_000.0,
    };
    let curve_env = game_balance::systems::upgrade_cost_curve::Env { levels: 10, gain_per_level: 0.05 };
    let prestige_env = game_balance::systems::reset_prestige::Env { session_goal_minutes: 20.0 };

    // Genre targets
    let tgt = IdleGenreTargets {
        ttu_target_secs: 30.0,
        util_target: 0.90,
        growth_target: 5.0,
        ttu_band_per_level: (7.5, 9.5),
        ttu_slope_pref: 1.15,
        prestige_cycle_minutes: 20.0,
        prestige_growth: 10.0,
        offline_retain_ratio: 0.70,
        typical_afk_minutes: 180.0,
    };

    let cfg = IdleGenreConfig { max_iters_per_system: 120_000, outer_iters: 1 };

    // Build hooks for this run (recreate per outer-iter if >1)
    let hooks = IdleGenreHooks { core_mechs: vec![core_mech] };

    let out = balance_idle_genre(core_env, curve_env, prestige_env, (), tgt, cfg, hooks);

    println!("== Idle + Draft Outcome ==");
    println!("Core   θ -> {:?}", out.core.theta);
    println!("Core   π -> {:?}", out.core.obs);
    println!("Curve  θ -> {:?}", out.curve.theta);
    println!("Curve  π -> {:?}", out.curve.obs);
    println!("Prest  θ -> {:?}", out.prestige.theta);
    println!("Prest  π -> {:?}", out.prestige.obs);
    println!("Offline θ -> {:?}", out.offline.theta);
    println!("Offline π -> {:?}", out.offline.obs);
}
