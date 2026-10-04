//! Primitives on flat `f64` slices: pricing, yields, durations, cash-flow
//! building, floater paths, tax and holding-period value.
//!
//! Conventions:
//! - Time is a day offset from the valuation date, ACT/365.
//! - Money amounts are absolute (per one bond, in currency units).
//! - Rates are annual effective fractions (0.12 = 12 percent) unless the
//!   parameter name ends with `_pct`, in which case they are percents.
//! - Cash flows are flat arrays so a WebAssembly boundary stays trivial.
//! - `min` and `max` propagate NaN, as JavaScript's `Math.min` and
//!   `Math.max` do, so a NaN input gives a NaN result in both
//!   implementations instead of being silently dropped.

/// Days in the year of the ACT/365 convention.
pub const YEAR: f64 = 365.0;

/// `build_cash_flow` offer mode: the offer is ignored.
pub const OFFER_NONE: u32 = 0;
/// `build_cash_flow` offer mode: the outstanding nominal is redeemed on the
/// first coupon day at or after the offer.
pub const OFFER_REDEEM: u32 = 1;
/// `build_cash_flow` offer mode: coupons after the offer pay the post-offer
/// rate.
pub const OFFER_RATE_CHANGE: u32 = 2;

/// `tax_amount` mode: coupons and positive capital gain are taxed.
pub const TAX_STANDARD: u32 = 0;
/// `tax_amount` mode: long-term holding relief (LDV).
pub const TAX_LDV: u32 = 1;
/// `tax_amount` mode: individual investment account of type B (IIS type B).
pub const TAX_IIS_B: u32 = 2;

pub(crate) fn max_nan(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

pub(crate) fn min_nan(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// Present value of the flows at an annual effective yield.
pub fn price_from_yield(amounts: &[f64], days: &[f64], y: f64) -> f64 {
    let base = 1.0 + y;
    let mut pv = 0.0;
    for (a, d) in amounts.iter().zip(days) {
        pv += a * base.powf(-d / YEAR);
    }
    pv
}

/// Annual effective yield that reproduces `price`. Bisection on a fixed
/// bracket (-99 to 1000 percent) with a fixed iteration count, so two
/// implementations converge to the same value. NaN when there are no flows
/// or the price is not positive.
pub fn ytm_effective(amounts: &[f64], days: &[f64], price: f64) -> f64 {
    if amounts.is_empty() || price.is_nan() || price <= 0.0 {
        return f64::NAN;
    }
    let mut lo = -0.99;
    let mut hi = 10.0;
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if price_from_yield(amounts, days, mid) > price {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Simple (non-compounded) annualised yield: total gain over price scaled
/// to a year by the day of the last flow. NaN when there are no flows, the
/// price is not positive or the last flow is not in the future.
pub fn ytm_simple(amounts: &[f64], days: &[f64], price: f64) -> f64 {
    if amounts.is_empty() || price.is_nan() || price <= 0.0 {
        return f64::NAN;
    }
    let total = amounts.iter().fold(0.0, |s, a| s + a);
    let last = days.iter().copied().fold(0.0, max_nan);
    if last <= 0.0 {
        return f64::NAN;
    }
    (total - price) / price * (YEAR / last)
}

/// Accrued coupon: the linear share of the coupon for the elapsed part of
/// the period. Zero when the period is not positive.
pub fn accrued_interest(coupon: f64, days_since_last: f64, period_days: f64) -> f64 {
    if period_days <= 0.0 {
        return 0.0;
    }
    coupon * max_nan(days_since_last, 0.0) / period_days
}

/// Macaulay duration in years at an annual effective yield. Zero when the
/// flows have no present value.
pub fn macaulay_duration(amounts: &[f64], days: &[f64], y: f64) -> f64 {
    let base = 1.0 + y;
    let mut pv_sum = 0.0;
    let mut weighted = 0.0;
    for (a, d) in amounts.iter().zip(days) {
        let t = d / YEAR;
        let pv = a * base.powf(-t);
        pv_sum += pv;
        weighted += t * pv;
    }
    if pv_sum == 0.0 {
        return 0.0;
    }
    weighted / pv_sum
}

/// Modified duration for an annual effective yield.
pub fn modified_duration(amounts: &[f64], days: &[f64], y: f64) -> f64 {
    macaulay_duration(amounts, days, y) / (1.0 + y)
}

/// Builds the payment schedule as flat triples `[day, coupon, principal]`.
///
/// - `coupon_days` are ascending coupon days; the last one is maturity.
/// - `coupon_rates_pct[i]` is the annual coupon rate for period `i`
///   (constant for fixed coupons, key rate plus spread for floaters); a
///   missing rate is zero.
/// - Amortisation entries pay `nominal * frac` on the coupon day within half
///   a day of theirs and reduce the base for later coupons. Whatever is
///   outstanding on the last day is repaid there.
/// - `offer_mode`: [`OFFER_NONE`], [`OFFER_REDEEM`] or
///   [`OFFER_RATE_CHANGE`] (rate `post_offer_rate_pct` after `offer_day`).
///   An `offer_day` that is not positive means no offer.
#[allow(clippy::too_many_arguments)]
pub fn build_cash_flow(
    nominal: f64,
    period_days: f64,
    coupon_days: &[f64],
    coupon_rates_pct: &[f64],
    amort_days: &[f64],
    amort_fracs: &[f64],
    offer_day: f64,
    offer_mode: u32,
    post_offer_rate_pct: f64,
) -> Vec<f64> {
    let n = coupon_days.len();
    let mut out = Vec::with_capacity(n * 3);
    let mut outstanding = nominal;
    let has_offer = offer_day > 0.0;
    for (i, &d) in coupon_days.iter().enumerate() {
        let mut rate = coupon_rates_pct.get(i).copied().unwrap_or(0.0);
        if offer_mode == OFFER_RATE_CHANGE && has_offer && d > offer_day {
            rate = post_offer_rate_pct;
        }
        let coupon = outstanding * rate / 100.0 * period_days / YEAR;
        let is_last = i + 1 == n;
        let is_offer = offer_mode == OFFER_REDEEM && has_offer && d >= offer_day;
        let principal = if is_last || is_offer {
            outstanding
        } else {
            let mut scheduled = 0.0;
            for (ad, f) in amort_days.iter().zip(amort_fracs) {
                if (ad - d).abs() < 0.5 {
                    scheduled += nominal * f;
                }
            }
            min_nan(scheduled, outstanding)
        };
        out.push(d);
        out.push(coupon);
        out.push(principal);
        outstanding -= principal;
        if is_last || is_offer {
            break;
        }
    }
    out
}

/// Key-rate path for a scenario: moves linearly from `base_pct` by
/// `delta_pct` over `ramp_steps` coupon periods, then stays flat.
pub fn floater_rate_path(base_pct: f64, delta_pct: f64, ramp_steps: u32, n: u32) -> Vec<f64> {
    let ramp = ramp_steps.max(1) as f64;
    (0..n)
        .map(|i| {
            let step = ((i + 1) as f64).min(ramp);
            base_pct + delta_pct * step / ramp
        })
        .collect()
}

/// Floater coupons: nominal times (key rate + spread) for each period.
pub fn floater_coupons(
    nominal: f64,
    spread_pct: f64,
    key_rates_pct: &[f64],
    period_days: f64,
) -> Vec<f64> {
    key_rates_pct
        .iter()
        .map(|k| nominal * (k + spread_pct) / 100.0 * period_days / YEAR)
        .collect()
}

/// Personal income tax on bond income.
///
/// - [`TAX_STANDARD`]: coupons and positive capital gain taxed at
///   `rate_pct`.
/// - [`TAX_LDV`]: the gain is exempt when the position is held for three
///   years (1,095 days) or more; coupons are still taxed.
/// - [`TAX_IIS_B`]: no tax at all.
///
/// Losses are not netted against coupons. An unknown mode taxes as
/// [`TAX_STANDARD`].
pub fn tax_amount(
    coupon_income: f64,
    capital_gain: f64,
    rate_pct: f64,
    mode: u32,
    hold_days: f64,
) -> f64 {
    if mode == TAX_IIS_B {
        return 0.0;
    }
    let gain_taxable = if mode == TAX_LDV && hold_days >= 3.0 * YEAR {
        0.0
    } else {
        max_nan(capital_gain, 0.0)
    };
    (max_nan(coupon_income, 0.0) + gain_taxable) * rate_pct / 100.0
}

/// What a holder collects by `horizon_day`, per one bond:
/// `[coupons, reinvest_income, amortisation, final_principal, sale_price]`.
///
/// Coupons paid on or before the horizon are collected and, when
/// `reinvest_rate > 0`, reinvested at that annual effective rate until the
/// horizon. Principal paid on the last flow day counts as final redemption,
/// earlier principal as amortisation. Flows after the horizon are sold as a
/// dirty price discounted to the horizon at `exit_yield`.
pub fn hold_value(
    days: &[f64],
    coupons: &[f64],
    principals: &[f64],
    horizon_day: f64,
    reinvest_rate: f64,
    exit_yield: f64,
) -> [f64; 5] {
    let last = days.iter().copied().fold(f64::NEG_INFINITY, max_nan);
    let mut coupons_sum = 0.0;
    let mut reinvest = 0.0;
    let mut amort = 0.0;
    let mut fin = 0.0;
    let mut sale = 0.0;
    for (i, &d) in days.iter().enumerate() {
        let c = coupons.get(i).copied().unwrap_or(0.0);
        let p = principals.get(i).copied().unwrap_or(0.0);
        if d <= horizon_day {
            coupons_sum += c;
            if reinvest_rate > 0.0 {
                reinvest += c * ((1.0 + reinvest_rate).powf((horizon_day - d) / YEAR) - 1.0);
            }
            if (d - last).abs() < 0.5 {
                fin += p;
            } else {
                amort += p;
            }
        } else {
            sale += (c + p) * (1.0 + exit_yield).powf(-(d - horizon_day) / YEAR);
        }
    }
    [coupons_sum, reinvest, amort, fin, sale]
}

/// First-order price change from a parallel shift of the rate curve:
/// `price * (1 - modified_duration * delta)`, floored at zero.
pub fn price_after_rate_shift(price: f64, mod_duration: f64, delta_pct: f64) -> f64 {
    max_nan(price * (1.0 - mod_duration * delta_pct / 100.0), 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytm_round_trip() {
        let amounts = [60.0, 60.0, 60.0, 1060.0];
        let days = [182.0, 364.0, 546.0, 728.0];
        let price = price_from_yield(&amounts, &days, 0.15);
        let y = ytm_effective(&amounts, &days, price);
        assert!((y - 0.15).abs() < 1e-9);
    }

    #[test]
    fn invalid_inputs_yield_nan() {
        assert!(ytm_effective(&[], &[], 100.0).is_nan());
        assert!(ytm_effective(&[100.0], &[365.0], 0.0).is_nan());
        assert!(ytm_effective(&[100.0], &[365.0], f64::NAN).is_nan());
        assert!(ytm_simple(&[100.0], &[0.0], 90.0).is_nan());
        assert!(accrued_interest(10.0, f64::NAN, 182.0).is_nan());
        assert!(tax_amount(f64::NAN, 0.0, 13.0, TAX_STANDARD, 10.0).is_nan());
        assert!(price_after_rate_shift(f64::NAN, 2.0, 1.0).is_nan());
    }

    #[test]
    fn nan_amortisation_is_not_dropped() {
        let flows = build_cash_flow(
            1000.0,
            100.0,
            &[100.0, 200.0],
            &[10.0, 10.0],
            &[100.0],
            &[f64::NAN],
            0.0,
            OFFER_NONE,
            0.0,
        );
        assert!(flows[2].is_nan());
    }
}
