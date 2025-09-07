// src/genres/sdk.rs

//! # Genre SDK
//!
//! This module provides small glue utilities for building **genres** out of
//! multiple **systems**.  
//!
//! ## When to create a new genre
//! A **genre** is a higher-level orchestrator that runs several systems together,
//! aligning their local equilibria (θ, π) into a consistent loop.  
//! For example, the `idle` genre coordinates:
//! - `production_spend` → reference income / utilization / TTU feel
//! - `upgrade_cost_curve` → per-level TTU pacing
//! - `reset_prestige` → meta-cycle timing and multiplier growth
//! - `offline_accumulation` → AFK retention
//!
//! If you want to model **autobattlers**, **roguelikes**, or **deckbuilders**,
//! you can define a new `genres/<name>.rs` orchestrator that pulls in whichever
//! systems are relevant and wires them together.
//!
//! ## Flexibility
//! - Genres are **freeform**: you decide what systems to include, what signals
//!   to pass between them, and how many outer iterations are needed.
//! - Systems are designed to be **neutral** (not hardcoded to a genre), so the
//!   same system (e.g. `upgrade_cost_curve`) can be reused in both `idle` and
//!   `roguelike` genres.
//! - `Signals` provides a light way to pass shared quantities (like reference
//!   income, cycle length, or winrate) between systems. Extend it only if you
//!   really need more fields.
//! - The `run_with_outer_iters` helper standardizes multi-pass balancing when
//!   you need systems to converge together. Each step returns both an `Outcome`
//!   and updated `Signals` for the next pass.
//!
//! ## Steps to add a new genre
//! 1. Create a new file under `src/genres/`, e.g. `roguelike.rs`.
//! 2. Import the systems you need (from `crate::systems::*`).
//! 3. Define a `Targets` struct for your genre (cross-system goals).
//! 4. Define a `Config` (max iters, outer loops, etc).
//! 5. Write an orchestrator function (like `balance_idle_genre`) that:
//!    - Seeds param guesses for each system.
//!    - Calls `balance_ext` on each system.
//!    - Threads `Signals` between them.
//!    - Returns a structured `Outcome` bundle.
//!
//! This keeps genres open-ended while still providing enough scaffolding for
//! consistency and reusability.

use crate::systems::sdk::Outcome;

/// Shared signals you may pass around between systems in a genre pass.
/// Add fields only when you actually need them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Signals {
    pub ref_income: f64,
}

/// Minimal step result to thread through the orchestrator loop.
#[derive(Clone, Debug)]
pub struct Step<TParams, TObs> {
    pub outcome: Outcome<TParams, TObs>,
    pub signals: Signals,
}

/// A tiny helper to standardize an outer loop. Each `step` does:
///   - run one or more systems
///   - compute/return updated Signals for the next step
pub fn run_with_outer_iters<F, TParams, TObs>(
    mut signals: Signals,
    outer_iters: usize,
    mut step: F,
) -> (Signals, Vec<Outcome<TParams, TObs>>)
where
    F: FnMut(Signals) -> (Signals, Outcome<TParams, TObs>),
{
    let mut outs = Vec::with_capacity(outer_iters);
    for _ in 0..outer_iters {
        let (s2, out) = step(signals);
        signals = s2;
        outs.push(out);
    }
    (signals, outs)
}
