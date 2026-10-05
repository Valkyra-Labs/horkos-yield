//! A plan for one issue: what the holder has at the horizon, the early exit
//! under a key-rate shift, floater scenarios and the offer pair.

use crate::issue::{derive_bond, CouponType, Derived, Error, Issue, Market, Schedule};
use crate::primitives::{
    build_cash_flow, floater_rate_path, hold_value, modified_duration, periodic_rate_pct,
    price_after_rate_shift, tax_amount, value_along_path, OFFER_NONE, OFFER_RATE_CHANGE, TAX_IIS_B,
    TAX_LDV, TAX_STANDARD, YEAR,
};

/// Brokerage commission in percent, charged on the purchase and on a sale
/// before redemption.
pub const COMMISSION_PCT: f64 = 0.05;
/// The coupon rate in percent assumed after an offer in the worst case.
pub const WORST_CASE_COUPON_PCT: f64 = 0.1;
/// Key-rate shifts in percentage points of the three floater scenarios.
pub const FLOATER_SHIFTS_PCT: [f64; 3] = [-2.0, 0.0, 2.0];
/// Coupon periods over which a floater scenario reaches its shift.
pub const FLOATER_RAMP_STEPS: u32 = 4;
/// The largest amount a plan accepts.
pub const MAX_AMOUNT: f64 = 1e9;

/// How income is taxed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaxRegime {
    /// Coupons and positive gain taxed at the plan's rate.
    Standard,
    /// Long-term holding relief: the gain is exempt after three years.
    Ldv,
    /// Individual investment account of type B: no tax.
    IisB,
}

impl TaxRegime {
    /// `"standard"`, `"ldv"` or `"iis_b"`.
    pub fn code(self) -> &'static str {
        match self {
            TaxRegime::Standard => "standard",
            TaxRegime::Ldv => "ldv",
            TaxRegime::IisB => "iis_b",
        }
    }

    /// The regime for a code, `None` for an unknown code.
    pub fn from_code(code: &str) -> Option<TaxRegime> {
        match code {
            "standard" => Some(TaxRegime::Standard),
            "ldv" => Some(TaxRegime::Ldv),
            "iis_b" => Some(TaxRegime::IisB),
            _ => None,
        }
    }

    fn mode(self) -> u32 {
        match self {
            TaxRegime::Standard => TAX_STANDARD,
            TaxRegime::Ldv => TAX_LDV,
            TaxRegime::IisB => TAX_IIS_B,
        }
    }
}

/// What the holder intends.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Money to invest; whole bonds are bought at the dirty price.
    pub amount: f64,
    /// Day offset of the horizon, from 1 to the maturity day.
    pub horizon_day: f64,
    /// Reinvest coupons at the yield to maturity until the horizon.
    pub reinvest: bool,
    pub tax_regime: TaxRegime,
    /// Tax rate in percent (13 or 15 for individuals).
    pub tax_rate_pct: f64,
    /// Key-rate change in percentage points by the horizon, for the early
    /// exit. A fixed coupon's sale price moves by its modified duration; a
    /// floater's coupons follow the key rate and its sale keeps the spread
    /// to the key rate.
    pub rate_shift_pct: f64,
}

/// Totals for the position. Income lines are positive, costs (`tax`,
/// `commission`) negative, so `total` is the sum of `coupons`, `reinvest`,
/// `amort`, `body`, `tax` and `commission`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breakdown {
    /// Bonds bought.
    pub qty: f64,
    /// Paid for them at the dirty price.
    pub invested: f64,
    pub coupons: f64,
    /// Income from reinvested coupons.
    pub reinvest: f64,
    /// Principal repaid before the final redemption.
    pub amort: f64,
    /// Final redemption plus the sale value at the horizon.
    pub body: f64,
    pub tax: f64,
    pub commission: f64,
    pub total: f64,
    /// `total - invested`.
    pub profit: f64,
    /// Effective annual return in percent, see [`effective_annual_pct`].
    pub annual_pct: f64,
    pub horizon_day: f64,
}

/// The plan's horizon with the key-rate shift applied to the sale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarlyExit {
    pub rate_shift_pct: f64,
    /// False when the horizon is the maturity day: nothing is sold.
    pub applicable: bool,
    pub result: Breakdown,
    /// `result.total - plan.total`.
    pub diff: f64,
    /// Modified duration of the flows after the horizon, at the horizon,
    /// for a fixed coupon. `None` for a floater, whose sale is not priced
    /// by duration.
    pub mod_duration_at_horizon: Option<f64>,
}

/// One key-rate scenario for a floater.
#[derive(Clone, Debug, PartialEq)]
pub struct FloaterScenario {
    pub shift_pct: f64,
    pub breakdown: Breakdown,
    /// Coupon per bond on each coupon day.
    pub coupons: Vec<f64>,
}

/// The three floater scenarios of [`FLOATER_SHIFTS_PCT`].
#[derive(Clone, Debug, PartialEq)]
pub struct FloaterScenarios {
    /// The coupon days the scenario coupons fall on.
    pub days: Vec<f64>,
    pub scenarios: Vec<FloaterScenario>,
}

/// Holding to the offer and redeeming there, against holding through it to
/// maturity at [`WORST_CASE_COUPON_PCT`] after the offer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OfferPair {
    pub before: Breakdown,
    pub after: Breakdown,
}

/// Everything a plan produces.
#[derive(Clone, Debug, PartialEq)]
pub struct Calculation {
    pub plan: Breakdown,
    pub early_exit: EarlyExit,
    /// Floaters only.
    pub floater: Option<FloaterScenarios>,
    /// Issues with an offer after the valuation date only.
    pub offer: Option<OfferPair>,
}

/// Effective annual return in percent: the rate a deposit would need to
/// turn `invested` into `total` over `horizon_day` days. Zero when either
/// is not positive, -100 when `total` is not positive.
pub fn effective_annual_pct(invested: f64, total: f64, horizon_day: f64) -> f64 {
    if invested.is_nan() || invested <= 0.0 || horizon_day.is_nan() || horizon_day <= 0.0 {
        return 0.0;
    }
    if total <= 0.0 {
        return -100.0;
    }
    ((total / invested).powf(YEAR / horizon_day) - 1.0) * 100.0
}

struct Hold<'a> {
    qty: f64,
    dirty_price: f64,
    period_days: f64,
    reinvest_rate: f64,
    exit_yield: f64,
    plan: &'a Plan,
}

/// How the flows after the horizon are sold.
enum Sale<'a> {
    /// At the exit yield, then moved by the modified duration for a
    /// key-rate shift in percentage points (a fixed coupon).
    Shifted(f64),
    /// Discounted along a path of per-period rates in percent (a floater).
    Path(&'a [f64]),
}

impl Hold<'_> {
    fn breakdown(&self, flows: &Schedule, horizon_day: f64, how: Sale) -> (Breakdown, Option<f64>) {
        let hv = hold_value(
            &flows.days,
            &flows.coupons,
            &flows.principals,
            horizon_day,
            self.reinvest_rate,
            self.exit_yield,
        );
        let mut sale = hv[4];
        let mut mod_duration_at_horizon = None;
        match how {
            Sale::Shifted(rate_shift_pct) => {
                let mut duration = 0.0;
                if sale > 0.0 {
                    let mut amounts = Vec::new();
                    let mut shifted = Vec::new();
                    for (i, &d) in flows.days.iter().enumerate() {
                        if d > horizon_day {
                            amounts.push(flows.coupons[i] + flows.principals[i]);
                            shifted.push(d - horizon_day);
                        }
                    }
                    duration = modified_duration(&amounts, &shifted, self.exit_yield);
                    if rate_shift_pct != 0.0 {
                        sale = price_after_rate_shift(sale, duration, rate_shift_pct);
                    }
                }
                mod_duration_at_horizon = Some(duration);
            }
            Sale::Path(rates_pct) => {
                if sale > 0.0 {
                    sale = value_along_path(
                        &flows.days,
                        &flows.amounts(),
                        horizon_day,
                        self.period_days,
                        rates_pct,
                    );
                }
            }
        }
        let qty = self.qty;
        let invested = qty * self.dirty_price;
        let coupons = hv[0] * qty;
        let reinvest = hv[1] * qty;
        let amort = hv[2] * qty;
        let body = (hv[3] + sale) * qty;
        let sold = if sale > 0.0 { sale * qty } else { 0.0 };
        let commission = (invested + sold) * COMMISSION_PCT / 100.0;
        let gain = amort + body - invested;
        let tax = tax_amount(
            coupons + reinvest,
            gain,
            self.plan.tax_rate_pct,
            self.plan.tax_regime.mode(),
            horizon_day,
        );
        let total = coupons + reinvest + amort + body - tax - commission;
        let breakdown = Breakdown {
            qty,
            invested,
            coupons,
            reinvest,
            amort,
            body,
            tax: -tax,
            commission: -commission,
            total,
            profit: total - invested,
            annual_pct: effective_annual_pct(invested, total, horizon_day),
            horizon_day,
        };
        (breakdown, mod_duration_at_horizon)
    }
}

/// Checks a plan against an issue's derived values, in the order
/// [`Error::AmountNotPositive`], [`Error::AmountTooLarge`],
/// [`Error::HorizonOutOfRange`], [`Error::InvalidPrice`],
/// [`Error::AmountBelowOneBond`], and returns the number of bonds bought.
fn check_plan(d: &Derived, plan: &Plan) -> Result<f64, Error> {
    if !plan.amount.is_finite() || plan.amount <= 0.0 {
        return Err(Error::AmountNotPositive);
    }
    if plan.amount > MAX_AMOUNT {
        return Err(Error::AmountTooLarge);
    }
    if !plan.horizon_day.is_finite() || plan.horizon_day < 1.0 || plan.horizon_day > d.maturity_day
    {
        return Err(Error::HorizonOutOfRange);
    }
    if !(d.dirty_price.is_finite() && d.dirty_price > 0.0) {
        return Err(Error::InvalidPrice);
    }
    let qty = (plan.amount / d.dirty_price).floor();
    if qty < 1.0 {
        return Err(Error::AmountBelowOneBond);
    }
    Ok(qty)
}

/// A floater's flows when the key rate moves by `shift_pct` in equal steps
/// on each of the first `steps` coupons and then stays, and the per-period
/// rates its flows are discounted at: the market's rate for the issue today
/// ([`periodic_rate_pct`] of the yield to maturity) moved by the same
/// change of the key rate, so the spread the market asks over the key rate
/// stays as it is today.
fn floater_path(
    issue: &Issue,
    market: &Market,
    d: &Derived,
    shift_pct: f64,
    steps: u32,
) -> (Schedule, Vec<f64>) {
    let key = floater_rate_path(
        market.key_rate_pct,
        shift_pct,
        steps,
        d.coupon_days.len() as u32,
    );
    let rates: Vec<f64> = key.iter().map(|k| k + issue.spread_pct).collect();
    let flows = Schedule::from_triples(&build_cash_flow(
        issue.nominal,
        issue.period_days,
        &d.coupon_days,
        &rates,
        &d.amort_days,
        &d.amort_fracs,
        0.0,
        OFFER_NONE,
        0.0,
    ));
    let today = periodic_rate_pct(d.ytm_maturity, issue.period_days);
    let discount = key
        .iter()
        .map(|k| today + (k - market.key_rate_pct))
        .collect();
    (flows, discount)
}

/// Calculates a plan for an issue: derives the issue (its errors come
/// first), checks the plan, then computes the plan's totals, the early exit
/// with the plan's key-rate change, the floater scenarios and the offer
/// pair.
///
/// Coupons are reinvested, when the plan asks, at the yield to maturity;
/// flows after the horizon are sold at the yield to maturity. Tax treats
/// the horizon as the holding period.
///
/// A key-rate change moves a fixed coupon's sale price by its modified
/// duration. A floater's coupons follow the key rate instead, and its sale
/// keeps today's spread to the key rate (see [`value_along_path`]), so its
/// price stays close to where it is: in the early exit the key rate moves
/// in equal steps on each coupon up to the first one after the horizon; in
/// the scenarios over [`FLOATER_RAMP_STEPS`] coupons.
pub fn calculate(issue: &Issue, market: &Market, plan: &Plan) -> Result<Calculation, Error> {
    let d = derive_bond(issue, market)?;
    let qty = check_plan(&d, plan)?;
    let y = d.ytm_maturity;
    let hold = Hold {
        qty,
        dirty_price: d.dirty_price,
        period_days: issue.period_days,
        reinvest_rate: if plan.reinvest { y } else { 0.0 },
        exit_yield: y,
        plan,
    };

    let (base, _) = hold.breakdown(&d.flows, plan.horizon_day, Sale::Shifted(0.0));
    let applicable = plan.horizon_day < d.maturity_day;
    let shift = if applicable { plan.rate_shift_pct } else { 0.0 };
    let (early, mod_duration_at_horizon) = match issue.coupon_type {
        CouponType::Fixed => hold.breakdown(&d.flows, plan.horizon_day, Sale::Shifted(shift)),
        // An unchanged key rate is the plan itself.
        CouponType::Floater if shift == 0.0 => (base, None),
        CouponType::Floater => {
            let paid = d
                .coupon_days
                .iter()
                .filter(|&&day| day <= plan.horizon_day)
                .count() as u32;
            let (flows, discount) = floater_path(issue, market, &d, shift, paid + 1);
            hold.breakdown(&flows, plan.horizon_day, Sale::Path(&discount))
        }
    };

    let floater = match issue.coupon_type {
        CouponType::Fixed => None,
        CouponType::Floater => {
            let scenarios = FLOATER_SHIFTS_PCT
                .iter()
                .map(|&shift_pct| {
                    let (flows, discount) =
                        floater_path(issue, market, &d, shift_pct, FLOATER_RAMP_STEPS);
                    // An unchanged key rate is the plan itself.
                    let how = if shift_pct == 0.0 {
                        Sale::Shifted(0.0)
                    } else {
                        Sale::Path(&discount)
                    };
                    let (breakdown, _) = hold.breakdown(&flows, plan.horizon_day, how);
                    FloaterScenario {
                        shift_pct,
                        breakdown,
                        coupons: flows.coupons,
                    }
                })
                .collect();
            Some(FloaterScenarios {
                days: d.coupon_days.clone(),
                scenarios,
            })
        }
    };

    let offer = match (d.offer_day, &d.flows_to_offer) {
        (Some(offer_day), Some(to_offer)) => {
            let (before, _) = hold.breakdown(to_offer, offer_day, Sale::Shifted(0.0));
            let worst = Schedule::from_triples(&build_cash_flow(
                issue.nominal,
                issue.period_days,
                &d.coupon_days,
                &d.rates_pct,
                &d.amort_days,
                &d.amort_fracs,
                offer_day,
                OFFER_RATE_CHANGE,
                WORST_CASE_COUPON_PCT,
            ));
            let (after, _) = hold.breakdown(&worst, d.maturity_day, Sale::Shifted(0.0));
            Some(OfferPair { before, after })
        }
        _ => None,
    };

    Ok(Calculation {
        plan: base,
        early_exit: EarlyExit {
            rate_shift_pct: plan.rate_shift_pct,
            applicable,
            result: early,
            diff: early.total - base.total,
            mod_duration_at_horizon,
        },
        floater,
        offer,
    })
}
