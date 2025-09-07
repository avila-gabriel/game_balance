/*!
`game_balance` — a minimal, pure closed-loop refinement harness.

What it does
- Orchestrates feedback refinement over parameters Θ around an opaque process.
- Composes caller-supplied pure functions
  (`simulate : Θ→D`, `measure : D→Π`, `update : Θ×Π→Θ`)
  into a single step `g(θ) = update(θ, measure(simulate(θ)))`.
- Repeats until a caller-supplied stopping predicate holds.

How to use (call surface only)
- Provide an initial parameter token `Params` as θ₀.
- Provide four pure functions with these signatures:
  * `simulate : &Params -> Data`
  * `measure  : &Data -> Metrics`
  * `update   : (&Params, &Metrics) -> Params`
  * `converged: (&Params, &Params) -> bool`
- Call `refine_det(θ₀, simulate, measure, update, converged, max_iters) -> Params`.

What it does NOT do
- No domain, no objectives, no randomness. You define those externally.
*/

#[derive(Clone, Debug)]
pub struct Params {}

#[derive(Clone, Debug)]
pub struct Data {}

#[derive(Clone, Debug)]
pub struct Metrics {}

/// Deterministic refinement: θ_{t+1} = update(θ_t, measure(simulate(θ_t))).
pub fn refine_det<Sim, Meas, Upd, Conv>(
    mut theta: Params,
    mut simulate: Sim,
    mut measure: Meas,
    mut update: Upd,
    converged: Conv,
    max_iters: usize,
) -> Params
where
    Sim: FnMut(&Params) -> Data,
    Meas: FnMut(&Data) -> Metrics,
    Upd: FnMut(&Params, &Metrics) -> Params,
    Conv: Fn(&Params, &Params) -> bool,
{
    for _ in 0..max_iters {
        let data = simulate(&theta);
        let pi = measure(&data);
        let theta_next = update(&theta, &pi);
        if converged(&theta, &theta_next) {
            return theta_next;
        }
        theta = theta_next;
    }
    theta
}

pub mod mechanics;
pub mod systems;
pub mod genres;