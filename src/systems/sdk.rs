// src/systems/sdk.rs

//! # Systems SDK
//!
//! Lightweight harness + hook protocol for building **systems** (self-contained
//! balancing loops such as production–spend, upgrade curve, prestige, etc.).
//! A *system* owns a local set of parameters `θ` and observables `π` and drives
//! `θ` toward caller-supplied targets using your pure closures.
//!
//! ## When to create a new system
//! Create a system when a mechanic can be tuned in relative isolation with a
//! clear set of inputs/outputs, e.g.:
//! - **production_spend**: target TTU/utilization/growth feel
//! - **upgrade_cost_curve**: target TTU band & slope across levels
//! - **reset_prestige**: target cycle time & meta growth
//! - **offline_accumulation**: target AFK retention
//!
//! Systems should be **genre-neutral** so they can be reused in multiple
//! genres (idle, roguelike, autobattler, …).
//!
//! ## What this SDK gives you
//! - A generic `balance_with_hooks` harness that wires your pure closures into
//!   the base refinement loop (`refine_det`).
//! - A small **hook** protocol (`Hook`) so optional sub-mechanics can
//!   participate without changing the core system (e.g., fees, caps, auras).
//! - A standard `Outcome<TParams, Obs>` return (θ, π, iters, converged).
//!
//! ## Your responsibilities (per system)
//! Implement the four closures required by `balance_with_hooks`:
//!
//! 1) **simulate**: `(&θ, &Env, &Tgt, &mut [Hook]) -> Obs`  
//!    - Compute observables `π` from current params `θ` and environment `Env`.  
//!    - You may let hooks modulate inputs (e.g., multiply income).
//!
//! 2) **nominal**: `(&θ, &Env, &Tgt, &Obs) -> NominalTargets`  
//!    - Convert `Obs` and `Tgt` into *pre-update* controller targets (x, y, z).
//!      Think “what should the controller try to hit this step?”
//!
//! 3) **step**: `(&θ, &Bounds, &Gains, NominalTargets, TargetAdjust) -> θ'`  
//!    - Move parameters toward (adjusted) targets using your controller
//!      (commonly proportional smoothing via `mechanics::control::approach`).
//!
//! 4) **converged**: `(&Obs, &Tgt) -> bool`  
//!    - Decide if `Obs` is within your acceptance band. Keep this tolerant to
//!      avoid oscillation; it’s a **band**, not an exact equality.
//!
//! ## Hooks (optional sub-mechanics)
//! Implement `Hook<TParams, Env, Tgt, Obs>` for pluggable effects:
//!
//! - `income_multiplier(base_income, θ, Env) -> f64`  
//!   Multiply a key input *inside simulate* (e.g., fees, buffs). Default 1.0.
//!
//! - `on_observe(&Obs, &θ, &Env, &Tgt)`  
//!   Observe/capture state post-sim (e.g., store smoothed metrics).
//!
//! - `adjust_targets(&θ, &Env, &Tgt, &NominalTargets) -> TargetAdjust`  
//!   Multiply controller’s nominal targets (x,y,z) by `(a,b,c)`; defaults to
//!   identity `(1,1,1)`. Use this for **policy**, not for re-simulating math.
//!
//! Hooks let you extend behavior without editing the system module.
//!
//! ## Determinism & purity
//! - Keep simulate/nominal/step **pure**. Side effects should be confined to
//!   hook internal caches or captured cells you control (if absolutely needed).
//! - Determinism is great for CI. If you add RNG, inject seeds explicitly.
//!
//! ## Bounds, Gains, Targets
//! - **Bounds**: clamp outputs of `step` to sane domains (stability & safety).
//! - **Gains**: choose gentle smoothing (0.4–0.7 typical). Raise only if your
//!   converge band is wide and the model is well-conditioned.
//! - **Targets**: represent **what you want**, not how to achieve it.
//!
//! ## Testing a system
//! - Unit tests at `tests/<system>.rs` that pin simple targets and assert
//!   convergence.  
//! - Example scenarios under `examples/` to show typical usage or hook stacks.
//!
//! ## Feature flags & reuse
//! - Keep systems under `src/systems/*` and gate with `feature = "system-*"`.
//! - Do not import genre-specific code here; genres compose systems, not vice versa.
//!
//! ## Anti-patterns to avoid
//! - Don’t duplicate simulate math again in hooks. Hooks should *modulate*,
//!   not *rebuild* the core model.
//! - Don’t tighten converge tolerances to zero; you’ll get chattering.
//!
//! With this SDK, adding a new system is mostly writing 100–200 LOC of clear,
//! pure math and control, plus a couple of tests and an example.

// -----------------------------------------------------------------------------
// Implementation
// -----------------------------------------------------------------------------

use std::cell::RefCell;
use std::rc::Rc;

use crate::{Data, Metrics, Params, refine_det};

/// Multiplicative target scalars (mechanics compose by multiplying).
#[derive(Clone, Copy, Debug)]
pub struct TargetAdjust {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}
impl TargetAdjust {
    pub fn id() -> Self { Self { a: 1.0, b: 1.0, c: 1.0 } }
}

/// What the controller is about to aim for (system computes this).
#[derive(Clone, Copy, Debug)]
pub struct NominalTargets {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A “mechanic” that can view observables, scale pre-update targets, etc.
pub trait Hook<TParams, Env, Tgt, Obs> {
    /// (Optional) multiply the base income inside simulate (default: 1.0).
    fn income_multiplier(&mut self, _base_income: f64, _theta: &TParams, _env: &Env) -> f64 {
        1.0
    }
    /// (Optional) let the hook observe/cache state after simulate.
    fn on_observe(&mut self, _obs: &Obs, _theta: &TParams, _env: &Env, _tgt: &Tgt) {}
    /// (Optional) multiplicative adjustment of controller’s nominal targets.
    fn adjust_targets(
        &mut self,
        _theta: &TParams,
        _env: &Env,
        _tgt: &Tgt,
        _nom: &NominalTargets,
    ) -> TargetAdjust {
        TargetAdjust::id()
    }
}

/// Generic result.
#[derive(Clone, Debug)]
pub struct Outcome<TParams, Obs> {
    pub theta: TParams,
    pub obs: Obs,
    pub iters: usize,
    pub converged: bool,
}

/// Generic harness for systems with hooks.
/// You provide 4 closures: simulate, nominal, step, converged.
pub fn balance_with_hooks<
    TParams: Clone,
    Env: Clone,
    Tgt: Clone,
    Bnd: Clone,
    G: Clone,
    Obs: Clone + Default + 'static,
>(
    theta0: TParams,
    env: Env,
    tgt: Tgt,
    bnd: Bnd,
    gains: G,
    hooks: Vec<Box<dyn Hook<TParams, Env, Tgt, Obs>>>,
    max_iters: usize,
    simulate: impl Fn(&TParams, &Env, &Tgt, &mut [Box<dyn Hook<TParams, Env, Tgt, Obs>>]) -> Obs + 'static,
    nominal: impl Fn(&TParams, &Env, &Tgt, &Obs) -> NominalTargets + 'static,
    step: impl Fn(&TParams, &Bnd, &G, NominalTargets, TargetAdjust) -> TParams + 'static,
    converged: impl Fn(&Obs, &Tgt) -> bool + 'static,
) -> Outcome<TParams, Obs> {
    let theta = Rc::new(RefCell::new(theta0));
    let obs   = Rc::new(RefCell::new(Obs::default()));
    let iters = Rc::new(RefCell::new(0usize));
    let done  = Rc::new(RefCell::new(false));
    let hooks_cell: Rc<RefCell<Vec<Box<dyn Hook<TParams, Env, Tgt, Obs>>>>> =
        Rc::new(RefCell::new(hooks));

    let simulate_cl = {
        let theta = Rc::clone(&theta);
        let obs   = Rc::clone(&obs);
        let env   = env.clone();
        let tgt   = tgt.clone();
        let hooks_cell = Rc::clone(&hooks_cell);
        move |_p: &Params| -> Data {
            let mut hs = hooks_cell.borrow_mut();
            let o = simulate(&theta.borrow(), &env, &tgt, &mut hs);
            *obs.borrow_mut() = o.clone();
            for h in hs.iter_mut() {
                h.on_observe(&o, &theta.borrow(), &env, &tgt);
            }
            Data {}
        }
    };

    let measure = |_d: &Data| Metrics {};

    let update_cl = {
        let theta = Rc::clone(&theta);
        let obs   = Rc::clone(&obs);
        let env   = env.clone();
        let tgt   = tgt.clone();
        let bnd   = bnd.clone();
        let gains = gains.clone();
        let hooks_cell = Rc::clone(&hooks_cell);
        move |_p: &Params, _m: &Metrics| -> Params {
            let th  = theta.borrow().clone();
            let o   = obs.borrow().clone();
            let nom = nominal(&th, &env, &tgt, &o);

            // Compose multiplicative adjustments from all hooks.
            let mut adj = TargetAdjust::id();
            {
                let mut hs = hooks_cell.borrow_mut();
                for h in hs.iter_mut() {
                    let s = h.adjust_targets(&th, &env, &tgt, &nom);
                    adj.a *= s.a.max(0.0);
                    adj.b *= s.b.max(0.0);
                    adj.c *= s.c.max(0.0);
                }
            }

            let next = step(&th, &bnd, &gains, nom, adj);
            *theta.borrow_mut() = next;
            Params {}
        }
    };

    let done_cl = {
        let obs   = Rc::clone(&obs);
        let iters = Rc::clone(&iters);
        let done  = Rc::clone(&done);
        let tgt   = tgt.clone();
        move |_a: &Params, _b: &Params| -> bool {
            *iters.borrow_mut() += 1;
            let ok = converged(&obs.borrow(), &tgt);
            if ok { *done.borrow_mut() = true; }
            ok
        }
    };

    let _ = refine_det(Params {}, simulate_cl, measure, update_cl, done_cl, max_iters);

    Outcome {
        theta: theta.borrow().clone(),
        obs:   obs.borrow().clone(),
        iters: *iters.borrow(),
        converged: *done.borrow(),
    }
}