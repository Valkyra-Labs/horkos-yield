//! JavaScript bindings (feature `wasm`).
//!
//! The primitives take and return `Float64Array`s and numbers under their
//! Rust names. `derive_bond` and `calculate` take and return wasm-bindgen
//! structs with camelCase fields (`Issue`, `Market` and `Plan` in;
//! `DeriveResult` and `CalculateResult` out, each with `ok` or `error`
//! set). Codes cross as strings: an unknown coupon type or tax regime is
//! the error `invalid_code`.
//!
//! Structs rather than JSON: on the 60-issue set the struct boundary
//! added 4 to 6 percent to the time spent inside wasm, the JSON boundary
//! 36 to 41 percent (docs/MEASUREMENTS.md).

use crate::primitives as p;
use crate::{
    Amortization, Breakdown, Calculation, CouponType, Derived, Error, Issue, Market, Plan,
    Schedule, TaxRegime,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn price_from_yield(amounts: &[f64], days: &[f64], y: f64) -> f64 {
    p::price_from_yield(amounts, days, y)
}

#[wasm_bindgen]
pub fn ytm_effective(amounts: &[f64], days: &[f64], price: f64) -> f64 {
    p::ytm_effective(amounts, days, price)
}

#[wasm_bindgen]
pub fn ytm_simple(amounts: &[f64], days: &[f64], price: f64) -> f64 {
    p::ytm_simple(amounts, days, price)
}

#[wasm_bindgen]
pub fn accrued_interest(coupon: f64, days_since_last: f64, period_days: f64) -> f64 {
    p::accrued_interest(coupon, days_since_last, period_days)
}

#[wasm_bindgen]
pub fn macaulay_duration(amounts: &[f64], days: &[f64], y: f64) -> f64 {
    p::macaulay_duration(amounts, days, y)
}

#[wasm_bindgen]
pub fn modified_duration(amounts: &[f64], days: &[f64], y: f64) -> f64 {
    p::modified_duration(amounts, days, y)
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
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
    p::build_cash_flow(
        nominal,
        period_days,
        coupon_days,
        coupon_rates_pct,
        amort_days,
        amort_fracs,
        offer_day,
        offer_mode,
        post_offer_rate_pct,
    )
}

#[wasm_bindgen]
pub fn floater_rate_path(base_pct: f64, delta_pct: f64, ramp_steps: u32, n: u32) -> Vec<f64> {
    p::floater_rate_path(base_pct, delta_pct, ramp_steps, n)
}

#[wasm_bindgen]
pub fn floater_coupons(
    nominal: f64,
    spread_pct: f64,
    key_rates_pct: &[f64],
    period_days: f64,
) -> Vec<f64> {
    p::floater_coupons(nominal, spread_pct, key_rates_pct, period_days)
}

#[wasm_bindgen]
pub fn tax_amount(
    coupon_income: f64,
    capital_gain: f64,
    rate_pct: f64,
    mode: u32,
    hold_days: f64,
) -> f64 {
    p::tax_amount(coupon_income, capital_gain, rate_pct, mode, hold_days)
}

#[wasm_bindgen]
pub fn hold_value(
    days: &[f64],
    coupons: &[f64],
    principals: &[f64],
    horizon_day: f64,
    reinvest_rate: f64,
    exit_yield: f64,
) -> Vec<f64> {
    p::hold_value(
        days,
        coupons,
        principals,
        horizon_day,
        reinvest_rate,
        exit_yield,
    )
    .to_vec()
}

#[wasm_bindgen]
pub fn periodic_rate_pct(y: f64, period_days: f64) -> f64 {
    p::periodic_rate_pct(y, period_days)
}

#[wasm_bindgen]
pub fn value_along_path(
    days: &[f64],
    amounts: &[f64],
    horizon_day: f64,
    period_days: f64,
    rates_pct: &[f64],
) -> f64 {
    p::value_along_path(days, amounts, horizon_day, period_days, rates_pct)
}

#[wasm_bindgen]
pub fn price_after_rate_shift(price: f64, mod_duration: f64, delta_pct: f64) -> f64 {
    p::price_after_rate_shift(price, mod_duration, delta_pct)
}

#[wasm_bindgen]
pub fn effective_annual_pct(invested: f64, total: f64, horizon_day: f64) -> f64 {
    crate::effective_annual_pct(invested, total, horizon_day)
}

#[wasm_bindgen(js_name = Issue, getter_with_clone)]
#[derive(Clone, Default)]
pub struct JsIssue {
    pub nominal: f64,
    #[wasm_bindgen(js_name = pricePct)]
    pub price_pct: f64,
    pub accrued: Option<f64>,
    #[wasm_bindgen(js_name = couponType)]
    pub coupon_type: String,
    #[wasm_bindgen(js_name = couponRatePct)]
    pub coupon_rate_pct: f64,
    #[wasm_bindgen(js_name = spreadPct)]
    pub spread_pct: f64,
    #[wasm_bindgen(js_name = periodDays)]
    pub period_days: f64,
    pub maturity: String,
    pub offers: Vec<String>,
    #[wasm_bindgen(js_name = amortDates)]
    pub amort_dates: Vec<String>,
    #[wasm_bindgen(js_name = amortFractionsPct)]
    pub amort_fractions_pct: Vec<f64>,
}

#[wasm_bindgen(js_class = Issue)]
impl JsIssue {
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsIssue {
        JsIssue::default()
    }
}

#[wasm_bindgen(js_name = Market, getter_with_clone)]
#[derive(Clone, Default)]
pub struct JsMarket {
    #[wasm_bindgen(js_name = valuationDate)]
    pub valuation_date: String,
    #[wasm_bindgen(js_name = keyRatePct)]
    pub key_rate_pct: f64,
}

#[wasm_bindgen(js_class = Market)]
impl JsMarket {
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsMarket {
        JsMarket::default()
    }
}

#[wasm_bindgen(js_name = Plan, getter_with_clone)]
#[derive(Clone, Default)]
pub struct JsPlan {
    pub amount: f64,
    #[wasm_bindgen(js_name = horizonDay)]
    pub horizon_day: f64,
    pub reinvest: bool,
    #[wasm_bindgen(js_name = taxRegime)]
    pub tax_regime: String,
    #[wasm_bindgen(js_name = taxRatePct)]
    pub tax_rate_pct: f64,
    #[wasm_bindgen(js_name = rateShiftPct)]
    pub rate_shift_pct: f64,
}

#[wasm_bindgen(js_class = Plan)]
impl JsPlan {
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsPlan {
        JsPlan::default()
    }
}

fn issue_of(i: &JsIssue) -> Result<Issue, Error> {
    Ok(Issue {
        nominal: i.nominal,
        price_pct: i.price_pct,
        accrued: i.accrued,
        coupon_type: CouponType::from_code(&i.coupon_type).ok_or(Error::InvalidCode)?,
        coupon_rate_pct: i.coupon_rate_pct,
        spread_pct: i.spread_pct,
        period_days: i.period_days,
        maturity: i.maturity.clone(),
        offers: i.offers.clone(),
        amortization: i
            .amort_dates
            .iter()
            .enumerate()
            .map(|(k, d)| Amortization {
                date: d.clone(),
                fraction_pct: i.amort_fractions_pct.get(k).copied().unwrap_or(f64::NAN),
            })
            .collect(),
    })
}

fn market_of(m: &JsMarket) -> Market {
    Market {
        valuation_date: m.valuation_date.clone(),
        key_rate_pct: m.key_rate_pct,
    }
}

fn plan_of(p: &JsPlan) -> Result<Plan, Error> {
    Ok(Plan {
        amount: p.amount,
        horizon_day: p.horizon_day,
        reinvest: p.reinvest,
        tax_regime: TaxRegime::from_code(&p.tax_regime).ok_or(Error::InvalidCode)?,
        tax_rate_pct: p.tax_rate_pct,
        rate_shift_pct: p.rate_shift_pct,
    })
}

#[wasm_bindgen(js_name = Schedule, getter_with_clone)]
#[derive(Clone)]
pub struct JsSchedule {
    pub days: Vec<f64>,
    pub coupons: Vec<f64>,
    pub principals: Vec<f64>,
}

impl From<&Schedule> for JsSchedule {
    fn from(s: &Schedule) -> JsSchedule {
        JsSchedule {
            days: s.days.clone(),
            coupons: s.coupons.clone(),
            principals: s.principals.clone(),
        }
    }
}

#[wasm_bindgen(js_name = Derived, getter_with_clone)]
#[derive(Clone)]
pub struct JsDerived {
    #[wasm_bindgen(js_name = maturityDay)]
    pub maturity_day: f64,
    #[wasm_bindgen(js_name = couponDays)]
    pub coupon_days: Vec<f64>,
    #[wasm_bindgen(js_name = ratesPct)]
    pub rates_pct: Vec<f64>,
    #[wasm_bindgen(js_name = amortDays)]
    pub amort_days: Vec<f64>,
    #[wasm_bindgen(js_name = amortFracs)]
    pub amort_fracs: Vec<f64>,
    #[wasm_bindgen(js_name = daysSinceLast)]
    pub days_since_last: f64,
    #[wasm_bindgen(js_name = couponAmount)]
    pub coupon_amount: f64,
    pub accrued: f64,
    #[wasm_bindgen(js_name = dirtyPrice)]
    pub dirty_price: f64,
    pub flows: JsSchedule,
    #[wasm_bindgen(js_name = flowsToOffer)]
    pub flows_to_offer: Option<JsSchedule>,
    #[wasm_bindgen(js_name = offerDay)]
    pub offer_day: Option<f64>,
    #[wasm_bindgen(js_name = ytmMaturity)]
    pub ytm_maturity: f64,
    #[wasm_bindgen(js_name = ytmOffer)]
    pub ytm_offer: Option<f64>,
    #[wasm_bindgen(js_name = ytmSimple)]
    pub ytm_simple: f64,
    pub event: String,
    #[wasm_bindgen(js_name = eventDay)]
    pub event_day: f64,
    #[wasm_bindgen(js_name = yieldEvent)]
    pub yield_event: f64,
    pub macaulay: f64,
    pub modified: f64,
}

impl From<Derived> for JsDerived {
    fn from(d: Derived) -> JsDerived {
        JsDerived {
            maturity_day: d.maturity_day,
            flows: (&d.flows).into(),
            flows_to_offer: d.flows_to_offer.as_ref().map(Into::into),
            coupon_days: d.coupon_days,
            rates_pct: d.rates_pct,
            amort_days: d.amort_days,
            amort_fracs: d.amort_fracs,
            days_since_last: d.days_since_last,
            coupon_amount: d.coupon_amount,
            accrued: d.accrued,
            dirty_price: d.dirty_price,
            offer_day: d.offer_day,
            ytm_maturity: d.ytm_maturity,
            ytm_offer: d.ytm_offer,
            ytm_simple: d.ytm_simple,
            event: d.event.code().to_owned(),
            event_day: d.event_day,
            yield_event: d.yield_event,
            macaulay: d.macaulay,
            modified: d.modified,
        }
    }
}

#[wasm_bindgen(js_name = DeriveResult, getter_with_clone)]
pub struct JsDeriveResult {
    pub ok: Option<JsDerived>,
    pub error: Option<String>,
}

#[wasm_bindgen(js_name = Breakdown)]
#[derive(Clone, Copy)]
pub struct JsBreakdown {
    pub qty: f64,
    pub invested: f64,
    pub coupons: f64,
    pub reinvest: f64,
    pub amort: f64,
    pub body: f64,
    pub tax: f64,
    pub commission: f64,
    pub total: f64,
    pub profit: f64,
    #[wasm_bindgen(js_name = annualPct)]
    pub annual_pct: f64,
    #[wasm_bindgen(js_name = horizonDay)]
    pub horizon_day: f64,
}

impl From<Breakdown> for JsBreakdown {
    fn from(b: Breakdown) -> JsBreakdown {
        JsBreakdown {
            qty: b.qty,
            invested: b.invested,
            coupons: b.coupons,
            reinvest: b.reinvest,
            amort: b.amort,
            body: b.body,
            tax: b.tax,
            commission: b.commission,
            total: b.total,
            profit: b.profit,
            annual_pct: b.annual_pct,
            horizon_day: b.horizon_day,
        }
    }
}

#[wasm_bindgen(js_name = EarlyExit)]
#[derive(Clone, Copy)]
pub struct JsEarlyExit {
    #[wasm_bindgen(js_name = rateShiftPct)]
    pub rate_shift_pct: f64,
    pub applicable: bool,
    pub result: JsBreakdown,
    pub diff: f64,
    #[wasm_bindgen(js_name = modDurationAtHorizon)]
    pub mod_duration_at_horizon: Option<f64>,
}

#[wasm_bindgen(js_name = FloaterScenario, getter_with_clone)]
#[derive(Clone)]
pub struct JsFloaterScenario {
    #[wasm_bindgen(js_name = shiftPct)]
    pub shift_pct: f64,
    pub breakdown: JsBreakdown,
    pub coupons: Vec<f64>,
}

#[wasm_bindgen(js_name = FloaterScenarios, getter_with_clone)]
#[derive(Clone)]
pub struct JsFloaterScenarios {
    pub days: Vec<f64>,
    pub scenarios: Vec<JsFloaterScenario>,
}

#[wasm_bindgen(js_name = OfferPair)]
#[derive(Clone, Copy)]
pub struct JsOfferPair {
    pub before: JsBreakdown,
    pub after: JsBreakdown,
}

#[wasm_bindgen(js_name = Calculation, getter_with_clone)]
#[derive(Clone)]
pub struct JsCalculation {
    pub plan: JsBreakdown,
    #[wasm_bindgen(js_name = earlyExit)]
    pub early_exit: JsEarlyExit,
    pub floater: Option<JsFloaterScenarios>,
    pub offer: Option<JsOfferPair>,
}

impl From<Calculation> for JsCalculation {
    fn from(c: Calculation) -> JsCalculation {
        JsCalculation {
            plan: c.plan.into(),
            early_exit: JsEarlyExit {
                rate_shift_pct: c.early_exit.rate_shift_pct,
                applicable: c.early_exit.applicable,
                result: c.early_exit.result.into(),
                diff: c.early_exit.diff,
                mod_duration_at_horizon: c.early_exit.mod_duration_at_horizon,
            },
            floater: c.floater.map(|f| JsFloaterScenarios {
                days: f.days,
                scenarios: f
                    .scenarios
                    .into_iter()
                    .map(|s| JsFloaterScenario {
                        shift_pct: s.shift_pct,
                        breakdown: s.breakdown.into(),
                        coupons: s.coupons,
                    })
                    .collect(),
            }),
            offer: c.offer.map(|o| JsOfferPair {
                before: o.before.into(),
                after: o.after.into(),
            }),
        }
    }
}

#[wasm_bindgen(js_name = CalculateResult, getter_with_clone)]
pub struct JsCalculateResult {
    pub ok: Option<JsCalculation>,
    pub error: Option<String>,
}

/// Derives an issue in a market.
#[wasm_bindgen]
pub fn derive_bond(issue: &JsIssue, market: &JsMarket) -> JsDeriveResult {
    match issue_of(issue).and_then(|i| crate::derive_bond(&i, &market_of(market))) {
        Ok(d) => JsDeriveResult {
            ok: Some(d.into()),
            error: None,
        },
        Err(e) => JsDeriveResult {
            ok: None,
            error: Some(e.code().to_owned()),
        },
    }
}

/// Calculates a plan for an issue in a market.
#[wasm_bindgen]
pub fn calculate(issue: &JsIssue, market: &JsMarket, plan: &JsPlan) -> JsCalculateResult {
    let r = issue_of(issue)
        .and_then(|i| Ok((i, plan_of(plan)?)))
        .and_then(|(i, pl)| crate::calculate(&i, &market_of(market), &pl));
    match r {
        Ok(c) => JsCalculateResult {
            ok: Some(c.into()),
            error: None,
        },
        Err(e) => JsCalculateResult {
            ok: None,
            error: Some(e.code().to_owned()),
        },
    }
}
