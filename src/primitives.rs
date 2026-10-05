//! Primitives on flat `f64` slices: pricing, yields, durations, cash-flow
//! building, floater paths, income tax and holding-period value.
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

/// Personal income tax rate on investment income up to [`TAX_THRESHOLD`]
/// a year, percent.
pub const TAX_RATE_PCT: f64 = 13.0;
/// The rate above [`TAX_THRESHOLD`], percent.
pub const TAX_HIGHER_RATE_PCT: f64 = 15.0;
/// Investment income in a calendar year taxed at [`TAX_RATE_PCT`]; the
/// rest of the year's investment income is taxed at
/// [`TAX_HIGHER_RATE_PCT`]. The threshold is shared by all of a person's
/// investment income: securities, dividends, deposit interest above the
/// tax-free allowance, individual investment accounts.
pub const TAX_THRESHOLD: f64 = 2_400_000.0;

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

/// Simple yield over the full term: all flows less the price, as a share
/// of the price, divided by the years to the last flow. Not compounded,
/// and it counts the money as invested until the last flow, so for an
/// amortising issue, whose principal comes back earlier, it is well below
/// the yield to maturity. NaN when there are no flows, the price is not
/// positive or the last flow is not in the future.
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

/// Personal income tax on one calendar year's taxable `base` from this
/// position, when the holder's other investment income that year is
/// `other_income`: [`TAX_RATE_PCT`] on the part that, added to the other
/// income, stays within [`TAX_THRESHOLD`], [`TAX_HIGHER_RATE_PCT`] on the
/// rest. A base that is not positive pays nothing.
pub fn income_tax(base: f64, other_income: f64) -> f64 {
    let base = max_nan(base, 0.0);
    let low = min_nan(max_nan(TAX_THRESHOLD - other_income, 0.0), base);
    low * TAX_RATE_PCT / 100.0 + (base - low) * TAX_HIGHER_RATE_PCT / 100.0
}

/// What a holder collects by `horizon_day`, per one bond:
/// `[coupons, reinvest_income, amortisation, final_principal, sale_price]`.
///
/// Coupons and principal paid on or before the horizon are collected and,
/// when `reinvest_rate > 0`, reinvested at that annual effective rate until
/// the horizon; `reinvest_income` is what that reinvestment earns.
/// Principal paid on the last flow day counts as final redemption, earlier
/// principal as amortisation. Flows after the horizon are sold as a dirty
/// price discounted to the horizon at `exit_yield`.
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
                reinvest += (c + p) * ((1.0 + reinvest_rate).powf((horizon_day - d) / YEAR) - 1.0);
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

/// The rate in percent, compounded once every `period_days`, that equals
/// the annual effective yield `y`: `((1 + y)^(period / 365) - 1) * 365 /
/// period * 100`. A floater's coupon is quoted in this convention, so the
/// difference between this rate and the key rate is the spread the market
/// asks of the issue.
pub fn periodic_rate_pct(y: f64, period_days: f64) -> f64 {
    ((1.0 + y).powf(period_days / YEAR) - 1.0) * YEAR / period_days * 100.0
}

/// Value at `horizon_day` of the flows after it, discounted period by
/// period: the flow on `days[i]` is discounted over its period at
/// `rates_pct[i]`, compounded once every `period_days`, and the first
/// period after the horizon only for the part of it that is left. With
/// the same rate in every period this is the value at the annual effective
/// yield that [`periodic_rate_pct`] converts from. `days` are the flow
/// days, ascending and a period apart; a missing rate is zero.
pub fn value_along_path(
    days: &[f64],
    amounts: &[f64],
    horizon_day: f64,
    period_days: f64,
    rates_pct: &[f64],
) -> f64 {
    let mut factor = 1.0;
    let mut from = horizon_day;
    let mut pv = 0.0;
    for (i, &d) in days.iter().enumerate() {
        if d <= horizon_day {
            continue;
        }
        let rate = rates_pct.get(i).copied().unwrap_or(0.0);
        factor *= (1.0 + rate / 100.0 * period_days / YEAR).powf(-(d - from) / period_days);
        from = d;
        pv += amounts.get(i).copied().unwrap_or(0.0) * factor;
    }
    pv
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
        assert!(income_tax(f64::NAN, 0.0).is_nan());
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
