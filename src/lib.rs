//! Bond mathematics for a bond-investing demo.
//!
//! [`primitives`]: pricing, yields, durations, cash-flow building with
//! amortisation and offers, floater rate paths and coupons, tax, holding
//! value and the price after a rate shift, on flat `f64` slices. Days are
//! offsets from the valuation date, ACT/365. Invalid inputs give NaN;
//! nothing panics on any input.

pub mod primitives;

pub use primitives::*;
