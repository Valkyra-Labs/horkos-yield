//! Bond mathematics for a bond-investing demo.
//!
//! Two layers:
//! - [`primitives`]: pricing, yields, durations, cash-flow building with
//!   amortisation and offers, floater rate paths and coupons, tax, holding
//!   value and the price after a rate shift, on flat `f64` slices.
//! - [`derive_bond`] and [`calculate`]: an issue and its market in, the
//!   schedule, yields and durations out; an issue and a plan in, the
//!   plan's totals, the early exit, floater scenarios and the offer pair
//!   out.
//!
//! Days are offsets from the valuation date, ACT/365. Invalid inputs give
//! a typed [`Error`] at the issue level and NaN in the primitives; nothing
//! panics on any input.
//!
//! A TypeScript twin in `twin/` implements the same functions
//! independently; `cases.json` and the parity tests hold the two together.
//!
//! ```
//! use horkos_yield::{calculate, derive_bond, CouponType, Issue, Market, Plan, TaxRegime};
//!
//! let issue = Issue {
//!     nominal: 1000.0,
//!     price_pct: 98.12,
//!     accrued: None,
//!     coupon_type: CouponType::Fixed,
//!     coupon_rate_pct: 14.0,
//!     spread_pct: 0.0,
//!     period_days: 182.0,
//!     maturity: "2029-01-12".into(),
//!     offers: vec![],
//!     amortization: vec![],
//! };
//! let market = Market { valuation_date: "2026-09-04".into(), key_rate_pct: 16.0 };
//!
//! let d = derive_bond(&issue, &market).unwrap();
//! assert_eq!(d.maturity_day, 861.0);
//! assert_eq!(d.coupon_days.len(), 5);
//!
//! let plan = Plan {
//!     amount: 100_000.0,
//!     horizon_day: 365.0,
//!     reinvest: true,
//!     tax_regime: TaxRegime::Standard,
//!     tax_rate_pct: 13.0,
//!     rate_shift_pct: 2.0,
//! };
//! let b = calculate(&issue, &market, &plan).unwrap().plan;
//! let lines = b.coupons + b.reinvest + b.amort + b.body + b.tax + b.commission;
//! assert!((lines - b.total).abs() < 1e-6);
//! ```

mod calculate;
pub mod date;
mod issue;
pub mod primitives;
#[cfg(feature = "wasm")]
mod wasm;

pub use calculate::{
    calculate, effective_annual_pct, Breakdown, Calculation, EarlyExit, FloaterScenario,
    FloaterScenarios, OfferPair, Plan, TaxRegime, COMMISSION_PCT, FLOATER_RAMP_STEPS,
    FLOATER_SHIFTS_PCT, MAX_AMOUNT, WORST_CASE_COUPON_PCT,
};
pub use issue::{
    coupon_schedule, derive_bond, Amortization, CouponType, Derived, Error, Event, Issue, Market,
    Schedule,
};
pub use primitives::*;
