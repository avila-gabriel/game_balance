// tests/core.rs
use game_balance::{Data, Metrics, Params, refine_det};
use std::cell::RefCell;
use std::rc::Rc;

/* ──────────────────────────────────────────────────────────────────────────
1) Matching Pennies — time-average (Cesàro) converges to 0.5 / 0.5
────────────────────────────────────────────────────────────────────────── */

/// Row payoff matrix A for Matching Pennies:
/// Rows = Row player's pure strategies (H,T), Cols = Column player's (H,T).
/// Row gets +1 if SAME, -1 if DIFFERENT. Zero-sum => Col gets -Row.
const A_MP: [[f64; 2]; 2] = [
    [1.0, -1.0], // Row:H vs Col:H,T
    [-1.0, 1.0], // Row:T vs Col:H,T
];

#[derive(Clone, Debug)]
struct Strat2 {
    h: f64,
    t: f64,
} // probability over {H,T}

impl Strat2 {
    fn new(h: f64) -> Self {
        let h = h.clamp(0.0, 1.0);
        Self { h, t: 1.0 - h }
    }
    fn l1(&self, other: &Strat2) -> f64 {
        (self.h - other.h).abs() + (self.t - other.t).abs()
    }
}

#[test]
fn matching_pennies_time_average_converges_to_half() {
    let p_row = Rc::new(RefCell::new(Strat2::new(0.9)));
    let p_col = Rc::new(RefCell::new(Strat2::new(0.1)));

    let avg_row = Rc::new(RefCell::new(Strat2::new(0.5)));
    let avg_col = Rc::new(RefCell::new(Strat2::new(0.5)));
    let steps = Rc::new(RefCell::new(0usize));

    let eta = 0.2_f64;
    let mu = 0.01_f64;

    let simulate = |_t: &Params| Data {};
    let measure = |_d: &Data| Metrics {};

    let update = {
        let p_row = Rc::clone(&p_row);
        let p_col = Rc::clone(&p_col);
        let avg_row = Rc::clone(&avg_row);
        let avg_col = Rc::clone(&avg_col);
        let steps = Rc::clone(&steps);

        move |_t: &Params, _m: &Metrics| -> Params {
            let pr = p_row.borrow().clone();
            let pc = p_col.borrow().clone();

            // u_row = A * q_col
            let u_r_h = A_MP[0][0] * pc.h + A_MP[0][1] * pc.t;
            let u_r_t = A_MP[1][0] * pc.h + A_MP[1][1] * pc.t;

            // u_col = -(A^T) * p_row  (since zero-sum)
            let u_c_h = -(A_MP[0][0] * pr.h + A_MP[1][0] * pr.t);
            let u_c_t = -(A_MP[0][1] * pr.h + A_MP[1][1] * pr.t);

            // MW step + uniform mutation
            let mut pr_h = pr.h * (eta * u_r_h).exp();
            let mut pr_t = pr.t * (eta * u_r_t).exp();
            let s_r = pr_h + pr_t;
            if s_r > 0.0 {
                pr_h /= s_r;
                pr_t /= s_r;
            }
            pr_h = (1.0 - mu) * pr_h + mu * 0.5;
            pr_t = (1.0 - mu) * pr_t + mu * 0.5;

            let mut pc_h = pc.h * (eta * u_c_h).exp();
            let mut pc_t = pc.t * (eta * u_c_t).exp();
            let s_c = pc_h + pc_t;
            if s_c > 0.0 {
                pc_h /= s_c;
                pc_t /= s_c;
            }
            pc_h = (1.0 - mu) * pc_h + mu * 0.5;
            pc_t = (1.0 - mu) * pc_t + mu * 0.5;

            *p_row.borrow_mut() = Strat2 { h: pr_h, t: pr_t };
            *p_col.borrow_mut() = Strat2 { h: pc_h, t: pc_t };

            // Update running averages
            let n = *steps.borrow() as f64;
            {
                let cur = p_row.borrow();
                let mut ar = avg_row.borrow_mut();
                ar.h = (ar.h * n + cur.h) / (n + 1.0);
                ar.t = 1.0 - ar.h;
            }
            {
                let cur = p_col.borrow();
                let mut ac = avg_col.borrow_mut();
                ac.h = (ac.h * n + cur.h) / (n + 1.0);
                ac.t = 1.0 - ac.h;
            }
            *steps.borrow_mut() += 1;

            Params {}
        }
    };

    let converged = {
        let avg_row = Rc::clone(&avg_row);
        let avg_col = Rc::clone(&avg_col);
        let steps = Rc::clone(&steps);
        move |_a: &Params, _b: &Params| -> bool {
            let target = Strat2::new(0.5);
            let ar = avg_row.borrow().clone();
            let ac = avg_col.borrow().clone();
            let close = ar.l1(&target) < 1e-3 && ac.l1(&target) < 1e-3;
            let enough_iters = *steps.borrow() > 50_000;
            close || enough_iters
        }
    };

    let _ = refine_det(Params {}, simulate, measure, update, converged, 200_000);

    let ar = avg_row.borrow().clone();
    let ac = avg_col.borrow().clone();
    let target = Strat2::new(0.5);
    assert!(
        ar.l1(&target) < 1e-3,
        "row time-average not at 0.5: {:?}",
        ar
    );
    assert!(
        ac.l1(&target) < 1e-3,
        "col time-average not at 0.5: {:?}",
        ac
    );
}

/* ──────────────────────────────────────────────────────────────────────────
2) RPS — gradient step toward uniform
────────────────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
struct Prob3 {
    r: f64,
    p: f64,
    s: f64,
}

impl Prob3 {
    fn normalize(mut self) -> Self {
        let sum = self.r + self.p + self.s;
        if sum > 0.0 {
            self.r /= sum;
            self.p /= sum;
            self.s /= sum;
        }
        self
    }
    fn l1(&self, other: &Prob3) -> f64 {
        (self.r - other.r).abs() + (self.p - other.p).abs() + (self.s - other.s).abs()
    }
}

fn step_toward_uniform(p: &Prob3, k: f64) -> Prob3 {
    let u = Prob3 {
        r: 1.0 / 3.0,
        p: 1.0 / 3.0,
        s: 1.0 / 3.0,
    };
    let dr = p.r - u.r;
    let dp = p.p - u.p;
    let ds = p.s - u.s;
    Prob3 {
        r: p.r - k * dr,
        p: p.p - k * dp,
        s: p.s - k * ds,
    }
    .normalize()
}

#[test]
fn rps_converges_to_uniform() {
    let p_state = Rc::new(RefCell::new(Prob3 {
        r: 0.8,
        p: 0.15,
        s: 0.05,
    }));

    let simulate = |_t: &Params| Data {};
    let measure = |_d: &Data| Metrics {};

    let update = {
        let p_state = Rc::clone(&p_state);
        move |_t: &Params, _m: &Metrics| -> Params {
            let old = p_state.borrow().clone();
            let newp = step_toward_uniform(&old, 0.2);
            *p_state.borrow_mut() = newp;
            Params {}
        }
    };

    let converged = {
        let p_state = Rc::clone(&p_state);
        move |_a: &Params, _b: &Params| -> bool {
            let cur = p_state.borrow();
            let u = Prob3 {
                r: 1.0 / 3.0,
                p: 1.0 / 3.0,
                s: 1.0 / 3.0,
            };
            cur.l1(&u) < 1e-6
        }
    };

    let _ = refine_det(Params {}, simulate, measure, update, converged, 10_000);

    let p_final = p_state.borrow().clone();
    let u = Prob3 {
        r: 1.0 / 3.0,
        p: 1.0 / 3.0,
        s: 1.0 / 3.0,
    };
    assert!(p_final.l1(&u) < 1e-6, "did not converge: {:?}", p_final);
}

/* ──────────────────────────────────────────────────────────────────────────
3) RPSLS — MW with uniform mutation → uniform
────────────────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
struct Prob5 {
    v: [f64; 5],
}

impl Prob5 {
    fn l1(&self, other: &Prob5) -> f64 {
        self.v
            .iter()
            .zip(other.v.iter())
            .map(|(a, b)| (a - b).abs())
            .sum()
    }
}

fn payoff_matrix_rpsls() -> [[f64; 5]; 5] {
    [
        [0.0, -1.0, 1.0, 1.0, -1.0], // Rock
        [1.0, 0.0, -1.0, -1.0, 1.0], // Paper
        [-1.0, 1.0, 0.0, 1.0, -1.0], // Scissors
        [-1.0, 1.0, -1.0, 0.0, 1.0], // Lizard
        [1.0, -1.0, 1.0, -1.0, 0.0], // Spock
    ]
}

fn mw_with_mutation(p: &Prob5, eta: f64, mu: f64, a: &[[f64; 5]; 5]) -> Prob5 {
    // u = A p
    let mut u = [0.0; 5];
    for i in 0..5 {
        for j in 0..5 {
            u[i] += a[i][j] * p.v[j];
        }
    }
    // multiplicative step: w_i ∝ p_i * exp(η u_i)
    let mut w = [0.0; 5];
    let mut sumw = 0.0;
    for i in 0..5 {
        w[i] = p.v[i] * (eta * u[i]).exp();
        sumw += w[i];
    }
    if sumw == 0.0 {
        return Prob5 { v: [0.2; 5] };
    }
    for i in 0..5 {
        w[i] /= sumw;
    } // normalize
    // mix with uniform
    let mut out = Prob5 { v: [0.0; 5] };
    for i in 0..5 {
        out.v[i] = (1.0 - mu) * w[i] + mu * 0.2;
    }
    out
}

#[test]
fn rpsls_converges_to_uniform() {
    let p_state = Rc::new(RefCell::new(Prob5 {
        v: [0.85, 0.05, 0.04, 0.03, 0.03],
    }));
    let a = payoff_matrix_rpsls();

    let eta = 0.2;
    let mu = 0.02;

    let simulate = |_t: &Params| Data {};
    let measure = |_d: &Data| Metrics {};

    let update = {
        let p_state = Rc::clone(&p_state);
        move |_t: &Params, _m: &Metrics| -> Params {
            let old = p_state.borrow().clone();
            let newp = mw_with_mutation(&old, eta, mu, &a);
            *p_state.borrow_mut() = newp;
            Params {}
        }
    };

    let converged = {
        let p_state = Rc::clone(&p_state);
        move |_a: &Params, _b: &Params| -> bool {
            let cur = p_state.borrow();
            let u = Prob5 {
                v: [0.2, 0.2, 0.2, 0.2, 0.2],
            };
            cur.l1(&u) < 1e-4
        }
    };

    let _ = refine_det(Params {}, simulate, measure, update, converged, 100_000);

    let p_final = p_state.borrow().clone();
    let u = Prob5 {
        v: [0.2, 0.2, 0.2, 0.2, 0.2],
    };
    assert!(p_final.l1(&u) < 1e-4, "did not converge: {:?}", p_final.v);
}

/* ──────────────────────────────────────────────────────────────────────────
4) TTK window — constant DPS vs opponent set
────────────────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
struct Stats {
    hp: f64,
    dps: f64,
} // our unit parameters

#[derive(Clone, Debug)]
struct Opp {
    hp: f64,
    dps: f64,
}

#[derive(Clone, Debug)]
struct MetricsTTK {
    avg_ttk: f64,
    avg_ttd: f64,
}

#[test]
fn ttk_converges_to_target_window() {
    let opponents = vec![
        Opp {
            hp: 500.0,
            dps: 50.0,
        },
        Opp {
            hp: 800.0,
            dps: 40.0,
        },
        Opp {
            hp: 1200.0,
            dps: 80.0,
        },
    ];

    let state = Rc::new(RefCell::new(Stats {
        hp: 600.0,
        dps: 60.0,
    }));
    let met = Rc::new(RefCell::new(MetricsTTK {
        avg_ttk: 0.0,
        avg_ttd: 0.0,
    }));

    let target_ttk = 8.0_f64;
    let target_ttd = 8.0_f64;

    let simulate = {
        let state = Rc::clone(&state);
        let opponents = opponents.clone();
        let met = Rc::clone(&met);
        move |_t: &Params| -> Data {
            let s = state.borrow().clone();
            let our_dps = s.dps.max(1e-6);
            let (mut sum_ttk, mut sum_ttd) = (0.0, 0.0);
            for o in &opponents {
                sum_ttk += o.hp / our_dps;
                sum_ttd += s.hp / o.dps.max(1e-6);
            }
            let n = opponents.len() as f64;
            met.borrow_mut().avg_ttk = sum_ttk / n;
            met.borrow_mut().avg_ttd = sum_ttd / n;
            Data {}
        }
    };

    let measure = |_d: &Data| Metrics {};

    let update = {
        let state = Rc::clone(&state);
        let met = Rc::clone(&met);

        // Small gains for stability
        let k_ttk_dps = -5.0_f64; // if avg_ttk too high (we kill slowly), increase dps
        let k_ttd_hp = -10.0_f64; // if avg_ttd too high (we live too long), reduce hp

        move |_t: &Params, _m: &Metrics| -> Params {
            let s_old = state.borrow().clone();
            let m = met.borrow().clone();

            let ttk_err = m.avg_ttk - target_ttk; // + => too slow killing
            let ttd_err = m.avg_ttd - target_ttd; // + => too survivable

            let mut hp = s_old.hp + k_ttd_hp * ttd_err;
            let mut dps = s_old.dps + (-k_ttk_dps) * ttk_err;

            hp = hp.clamp(100.0, 10_000.0);
            dps = dps.clamp(5.0, 5_000.0);

            *state.borrow_mut() = Stats { hp, dps };
            Params {}
        }
    };

    let converged = {
        let met = Rc::clone(&met);
        move |_a: &Params, _b: &Params| -> bool {
            let m = met.borrow();
            (m.avg_ttk - 8.0).abs() < 1e-3 && (m.avg_ttd - 8.0).abs() < 1e-3
        }
    };

    let _ = refine_det(Params {}, simulate, measure, update, converged, 200_000);

    let m = met.borrow().clone();
    assert!(
        (m.avg_ttk - 8.0).abs() < 1e-3,
        "avg_ttk not at target: {}",
        m.avg_ttk
    );
    assert!(
        (m.avg_ttd - 8.0).abs() < 1e-3,
        "avg_ttd not at target: {}",
        m.avg_ttd
    );

    let s = state.borrow().clone();
    assert!(
        s.hp.is_finite() && s.dps.is_finite(),
        "non-finite params {:?}",
        s
    );
}
