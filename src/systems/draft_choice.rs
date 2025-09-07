//! Draft choice (mechanics-aware) — factory-closure no longer cloned.
//! We store only `pool_idx` in the offered card; when picking, we
//! instantiate via the pool.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use bevy_prng::WyRand;
use rand_core::SeedableRng;

use crate::mechanics::{control, stoch};
use crate::systems::sdk::Hook;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier { Common = 0, Uncommon = 1, Rare = 2, Epic = 3 }

pub struct EffectCard<TParams, Env, Tgt, Obs> {
    pub name: String,
    pub tier: Tier,
    pub base_p: f64,
    pub pity: Option<PitySpec>,
    pub mk: Box<dyn Fn() -> Box<dyn Hook<TParams, Env, Tgt, Obs>>>,
}

#[derive(Clone, Copy, Debug)]
pub struct PitySpec {
    pub pity_cap: f64,
    pub k: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct DraftConfig {
    pub options_per_roll: usize,
    pub rerolls_per_draft: usize,
    pub prioritize_tier: bool,
}

pub struct DraftState {
    rng: Rc<RefCell<WyRand>>,
    pub rerolls_left: usize,
    pity_acc: Vec<f64>,
    last_offered_pool_idxs: Vec<usize>,
}

impl DraftState {
    pub fn new(cfg: DraftConfig, pool_len: usize, seed: u64) -> Self {
        Self {
            rng: Rc::new(RefCell::new(WyRand::from_seed(seed.to_le_bytes()))),
            rerolls_left: cfg.rerolls_per_draft,
            pity_acc: vec![0.0; pool_len],
            last_offered_pool_idxs: Vec::new(),
        }
    }
    pub fn resize_pool(&mut self, new_len: usize) {
        if new_len > self.pity_acc.len() {
            self.pity_acc.resize(new_len, 0.0);
        } else {
            self.pity_acc.truncate(new_len);
        }
    }
}

/// What we present to the player (no closure here).
pub struct OfferedCard {
    pub pool_idx: usize,
    pub name: String,
    pub tier: Tier,
}

pub fn make_offer<TParams, Env, Tgt, Obs>(
    pool: &[EffectCard<TParams, Env, Tgt, Obs>],
    cfg: DraftConfig,
    st: &mut DraftState,
) -> Vec<OfferedCard> {
    let mut candidates: Vec<(usize, &EffectCard<TParams, Env, Tgt, Obs>)> = Vec::new();
    for (i, e) in pool.iter().enumerate() {
        let base = e.base_p.clamp(0.0, 1.0);
        let boost = st.pity_acc.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let p = (base + boost).clamp(0.0, 1.0);
        if stoch::bernoulli(&st.rng, p) {
            candidates.push((i, e));
        }
    }

    if candidates.is_empty() && !pool.is_empty() {
        if let Some(idx) = pool.iter().position(|e| e.tier == Tier::Common) {
            candidates.push((idx, &pool[idx]));
        } else {
            candidates.push((0, &pool[0]));
        }
    }

    if cfg.prioritize_tier {
        candidates.sort_by_key(|(_, e)| std::cmp::Reverse(e.tier as i32));
        let mut i = 0;
        while i < candidates.len() {
            let t = candidates[i].1.tier;
            let mut j = i + 1;
            while j < candidates.len() && candidates[j].1.tier == t { j += 1; }

            // shuffle [i, j) using gaussian noise
            let mut with_noise: Vec<(f64, (usize, &EffectCard<TParams, Env, Tgt, Obs>))> =
                candidates[i..j].iter().cloned()
                    .map(|x| (stoch::gaussian01(&st.rng), x))
                    .collect();
            with_noise.sort_by(|a,b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            for (k, (_, v)) in with_noise.into_iter().enumerate() {
                candidates[i + k] = v;
            }
            i = j;
        }
    } else {
        let mut tagged: Vec<(f64, (usize, &EffectCard<TParams, Env, Tgt, Obs>))> =
            candidates.into_iter()
                .map(|x| (stoch::gaussian01(&st.rng), x))
                .collect();
        tagged.sort_by(|a,b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        candidates = tagged.into_iter().map(|(_, v)| v).collect();
    }

    let take = cfg.options_per_roll.max(1);
    let offer = candidates.into_iter().take(take).map(|(pool_idx, e)| {
        OfferedCard { pool_idx, name: e.name.clone(), tier: e.tier }
    }).collect::<Vec<_>>();

    st.last_offered_pool_idxs = offer.iter().map(|c| c.pool_idx).collect();
    apply_pity_after_offer(pool, st);

    offer
}

pub fn reroll_offer<TParams, Env, Tgt, Obs>(
    pool: &[EffectCard<TParams, Env, Tgt, Obs>],
    cfg: DraftConfig,
    st: &mut DraftState,
) -> Option<Vec<OfferedCard>> {
    if st.rerolls_left == 0 { return None; }
    st.rerolls_left -= 1;
    Some(make_offer(pool, cfg, st))
}

/// Instantiate the picked card into a concrete Hook by looking up its factory in the pool.
pub fn instantiate_hook<TParams, Env, Tgt, Obs>(
    pool: &[EffectCard<TParams, Env, Tgt, Obs>],
    card: &OfferedCard,
) -> Box<dyn Hook<TParams, Env, Tgt, Obs>> {
    (pool[card.pool_idx].mk)()
}

pub fn notify_picked<TParams, Env, Tgt, Obs>(
    pool: &[EffectCard<TParams, Env, Tgt, Obs>],
    st: &mut DraftState,
    offer: &[OfferedCard],
    picked_offer_idx: usize,
) {
    if let Some(chosen) = offer.get(picked_offer_idx) {
        if let Some(e) = pool.get(chosen.pool_idx) {
            if e.pity.is_some() {
                if let Some(p) = st.pity_acc.get_mut(chosen.pool_idx) {
                    *p = 0.0;
                }
            }
        }
    }
}

/* --- internal pity update --- */

fn apply_pity_after_offer<TParams, Env, Tgt, Obs>(
    pool: &[EffectCard<TParams, Env, Tgt, Obs>],
    st: &mut DraftState,
) {
    let shown: HashSet<usize> = st.last_offered_pool_idxs.iter().copied().collect();
    for (i, e) in pool.iter().enumerate() {
        if let Some(spec) = e.pity {
            let acc = &mut st.pity_acc[i];
            if shown.contains(&i) {
                // if shown, softly reset toward 0
                *acc = control::approach(*acc, 0.0, 1.0, 0.0, spec.pity_cap.max(0.0));
            } else {
                // if not shown, drift toward cap
                *acc = control::approach(
                    *acc,
                    spec.pity_cap.max(0.0),
                    spec.k.clamp(0.0, 1.0),
                    0.0,
                    spec.pity_cap.max(0.0),
                );
            }
        }
    }
}
