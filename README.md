# 🎮 Game Balance Sandbox  

A Rust crate for **systematic game balancing**.  

This is not a game engine. It’s a **balancing framework**:  
- Encode systems (idle production, upgrade cost curves, prestige, offline accumulation, draft choices, etc.) as small self-contained modules.  
- Define `Params`, `Env`, `Targets`, `Obs`.  
- Let the balancer iterate until KPIs (TTU, utilization, WR, etc.) converge.  

Think of it as a **sandbox for exploring mechanics**, not a ready-to-ship library.  

---

## ✨ What it does  

- Provides **systems** like:  
  - `production_spend` → generator vs. spend rate → utilization, surplus, TTU.  
  - `upgrade_cost_curve` → exponential growth tuned to stay inside a TTU band.  
  - `reset_prestige` → cycle length and reward scaling.  
  - `offline_accumulation` → AFK retention curve.  
  - `draft_choice` → roguelite-style effect selection.  

- Provides **genres** (example orchestrators) like:  
  - `idle` → stitches together production, curve, prestige, offline into a coherent idle loop.  

- Uses **mechanics** (math helpers) for economy, energy, storage, stochastic RNG, fees, win-rate, etc.  
  - These are the small Lego bricks you can reuse in your own systems.  

- Comes with **examples** (`cargo run --example idle`) that show a “playable” loop in numbers.  

---

## 🚀 How to use  

```
# Check it compiles
cargo check --all-features

# Run the idle example
cargo run --example idle \
  --features "genre-idle system-production_spend system-upgrade_cost_curve system-reset_prestige system-offline_accumulation"
```

You’ll see a readout of system parameters (θ) and observed KPIs (π) after balancing.  

---

## 🧩 How to extend  

Everything is designed to be **extensible**:  

1. **Add a new system**  
   - Create a `src/systems/my_system.rs`.  
   - Define:  
     ```rust
     pub struct Params { … }
     pub struct Env { … }
     pub struct Targets { … }
     pub struct Obs { … }
     pub fn balance_ext(…) -> Outcome<Params, Obs> { … }
     ```  
   - Wire it into `src/systems/mod.rs`.  

2. **Add a new genre**  
   - Genres are orchestrators that run multiple systems in sequence.  
   - Look at `src/genres/idle.rs` as a template.  
   - Define your own `Targets`, run each system, pass signals downstream.  

3. **Add new mechanics**  
   - If you find yourself writing little math helpers (caps, growth, RNG), put them in `src/mechanics`.  

4. **Experiment**  
   - Drop into `examples/` with a headless run.  
   - Plot CSV or print KPIs.  
   - Hack away.  

---

## 📜 License  

MIT. Do whatever you want.  
