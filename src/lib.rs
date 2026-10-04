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
