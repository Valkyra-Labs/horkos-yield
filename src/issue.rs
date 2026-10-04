//! An issue and what follows from it: the schedule, the cash flows to
//! maturity and to the nearest offer, accrued interest, yields and
//! durations.

use crate::date::parse_iso_date;
use crate::primitives::{
    accrued_interest, build_cash_flow, macaulay_duration, modified_duration, ytm_effective,
    ytm_simple, OFFER_NONE, OFFER_REDEEM, YEAR,
};
use std::fmt;

/// Why an issue or a plan cannot be calculated. Each variant has a stable
/// code, the same in the TypeScript twin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A coupon type or tax regime code is not one of the known codes.
    /// Only the JavaScript boundary and the twin produce it: the Rust types
    /// cannot hold an unknown code. It is checked before anything else.
    InvalidCode,
    /// A date is not a valid `YYYY-MM-DD`.
    InvalidDate,
    /// The nominal is not a positive finite number.
    InvalidNominal,
    /// The coupon period is not a finite number of at least one day.
    InvalidPeriod,
    /// The issue matures on or before the valuation date.
    Matured,
    /// The amount to invest is not a positive finite number.
    AmountNotPositive,
    /// The amount to invest is above [`crate::MAX_AMOUNT`].
    AmountTooLarge,
    /// The horizon is not between day 1 and the maturity day.
    HorizonOutOfRange,
    /// The dirty price is not a positive finite number.
    InvalidPrice,
    /// The amount does not buy a single bond.
    AmountBelowOneBond,
}

impl Error {
    /// The stable code of this error.
    pub fn code(self) -> &'static str {
        match self {
            Error::InvalidCode => "invalid_code",
            Error::InvalidDate => "invalid_date",
            Error::InvalidNominal => "invalid_nominal",
            Error::InvalidPeriod => "invalid_period",
            Error::Matured => "matured",
            Error::AmountNotPositive => "amount_not_positive",
            Error::AmountTooLarge => "amount_too_large",
            Error::HorizonOutOfRange => "horizon_out_of_range",
            Error::InvalidPrice => "invalid_price",
            Error::AmountBelowOneBond => "amount_below_one_bond",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for Error {}

/// How the coupon rate is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CouponType {
    /// `coupon_rate_pct` for every period.
    Fixed,
    /// The market key rate plus `spread_pct`.
    Floater,
}

impl CouponType {
    /// `"fixed"` or `"floater"`.
    pub fn code(self) -> &'static str {
        match self {
            CouponType::Fixed => "fixed",
            CouponType::Floater => "floater",
        }
    }

    /// The coupon type for a code, `None` for an unknown code.
    pub fn from_code(code: &str) -> Option<CouponType> {
        match code {
            "fixed" => Some(CouponType::Fixed),
            "floater" => Some(CouponType::Floater),
            _ => None,
        }
    }
}

/// One scheduled partial redemption.
#[derive(Clone, Debug, PartialEq)]
pub struct Amortization {
    /// `YYYY-MM-DD`; it pays only when it falls on a coupon day.
    pub date: String,
    /// Share of the original nominal, in percent.
    pub fraction_pct: f64,
}

/// A bond issue as quoted.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    /// Nominal per bond, in currency units.
    pub nominal: f64,
    /// Clean price in percent of nominal.
    pub price_pct: f64,
    /// Quoted accrued interest per bond. `None` uses the value computed
    /// from the schedule.
    pub accrued: Option<f64>,
    pub coupon_type: CouponType,
    /// Annual coupon rate in percent; used for fixed coupons only.
    pub coupon_rate_pct: f64,
    /// Spread over the key rate in percent; used for floaters only.
    pub spread_pct: f64,
    /// Days between coupons.
    pub period_days: f64,
    /// `YYYY-MM-DD`.
    pub maturity: String,
    /// Offer dates, `YYYY-MM-DD`, in any order.
    pub offers: Vec<String>,
    pub amortization: Vec<Amortization>,
}

/// The market the issue is valued in.
#[derive(Clone, Debug, PartialEq)]
pub struct Market {
    /// `YYYY-MM-DD`; every day offset counts from here.
    pub valuation_date: String,
    /// Key rate in percent; floaters pay it plus their spread.
    pub key_rate_pct: f64,
}

/// Cash flows per bond, one entry per payment day.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schedule {
    pub days: Vec<f64>,
    pub coupons: Vec<f64>,
    pub principals: Vec<f64>,
}

impl Schedule {
    /// From the flat `[day, coupon, principal]` triples of
    /// [`build_cash_flow`].
    pub fn from_triples(triples: &[f64]) -> Schedule {
        let mut s = Schedule::default();
        for t in triples.chunks_exact(3) {
            s.days.push(t[0]);
            s.coupons.push(t[1]);
            s.principals.push(t[2]);
        }
        s
    }

    /// Coupon plus principal for each day.
    pub fn amounts(&self) -> Vec<f64> {
        self.coupons
            .iter()
            .zip(&self.principals)
            .map(|(c, p)| c + p)
            .collect()
    }
}

/// The nearest redemption event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Offer,
    Maturity,
}

impl Event {
    /// `"offer"` or `"maturity"`.
    pub fn code(self) -> &'static str {
        match self {
            Event::Offer => "offer",
            Event::Maturity => "maturity",
        }
    }
}

/// What follows from an issue on the valuation date. Days are offsets from
/// the valuation date.
#[derive(Clone, Debug, PartialEq)]
pub struct Derived {
    pub maturity_day: f64,
    /// Coupon days after the valuation date, ascending; the last is
    /// maturity.
    pub coupon_days: Vec<f64>,
    /// Coupon rate in percent for each coupon day, at today's key rate for
    /// floaters.
    pub rates_pct: Vec<f64>,
    /// Amortisation days before maturity, and the share of nominal each
    /// pays as a fraction.
    pub amort_days: Vec<f64>,
    pub amort_fracs: Vec<f64>,
    /// Days since the last coupon (the period before the first coupon day).
    pub days_since_last: f64,
    /// The next coupon on the full nominal.
    pub coupon_amount: f64,
    /// Accrued interest computed from the schedule.
    pub accrued: f64,
    /// Clean price plus accrued interest (quoted when the issue gives it).
    pub dirty_price: f64,
    /// Flows to maturity.
    pub flows: Schedule,
    /// Flows when the bond is redeemed at the nearest offer.
    pub flows_to_offer: Option<Schedule>,
    /// The nearest offer after the valuation date.
    pub offer_day: Option<f64>,
    /// Annual effective yield to maturity at the dirty price (NaN when the
    /// price is not positive).
    pub ytm_maturity: f64,
    /// Annual effective yield to the nearest offer.
    pub ytm_offer: Option<f64>,
    /// Simple annualised yield to maturity.
    pub ytm_simple: f64,
    /// The nearest redemption event: the offer when there is one.
    pub event: Event,
    pub event_day: f64,
    /// Yield to that event.
    pub yield_event: f64,
    /// Macaulay and modified duration to maturity, in years.
    pub macaulay: f64,
    pub modified: f64,
}

/// Coupon days counted back from maturity in steps of the period, kept
/// while they are after the valuation date, ascending.
pub fn coupon_schedule(maturity_day: f64, period_days: f64) -> Vec<f64> {
    let mut days = Vec::new();
    let mut d = maturity_day;
    while d > 0.0 {
        days.push(d);
        d -= period_days;
    }
    days.reverse();
    days
}

pub(crate) fn base_rates(issue: &Issue, market: &Market, n: usize) -> Vec<f64> {
    let rate = match issue.coupon_type {
        CouponType::Fixed => issue.coupon_rate_pct,
        CouponType::Floater => market.key_rate_pct + issue.spread_pct,
    };
    vec![rate; n]
}

/// Derives the schedule, flows, accrued interest, yields to maturity and to
/// the nearest offer, and durations of an issue.
///
/// Errors, checked in this order: [`Error::InvalidDate`] (valuation date,
/// maturity, offers, amortisation dates), [`Error::InvalidNominal`],
/// [`Error::InvalidPeriod`], [`Error::Matured`]. A price that is not
/// positive is not an error: the yields are NaN, as the primitives
/// document.
///
/// Amortisation dates on or after maturity are dropped; the rest pay only
/// when they fall on a coupon day after the valuation date. Past
/// amortisation does not reduce the nominal: `nominal` is taken as
/// outstanding.
pub fn derive_bond(issue: &Issue, market: &Market) -> Result<Derived, Error> {
    let today = parse_iso_date(&market.valuation_date).ok_or(Error::InvalidDate)?;
    let day = |s: &str| -> Result<f64, Error> {
        parse_iso_date(s)
            .map(|d| (d - today) as f64)
            .ok_or(Error::InvalidDate)
    };
    let maturity_day = day(&issue.maturity)?;
    let mut offer_days = Vec::with_capacity(issue.offers.len());
    for o in &issue.offers {
        offer_days.push(day(o)?);
    }
    let mut amort = Vec::with_capacity(issue.amortization.len());
    for a in &issue.amortization {
        amort.push((day(&a.date)?, a.fraction_pct));
    }
    if !(issue.nominal.is_finite() && issue.nominal > 0.0) {
        return Err(Error::InvalidNominal);
    }
    if !(issue.period_days.is_finite() && issue.period_days >= 1.0) {
        return Err(Error::InvalidPeriod);
    }
    if maturity_day <= 0.0 {
        return Err(Error::Matured);
    }

    let period = issue.period_days;
    let coupon_days = coupon_schedule(maturity_day, period);
    let first = coupon_days.first().copied().unwrap_or(maturity_day);
    let days_since_last = period - first;
    let rates_pct = base_rates(issue, market, coupon_days.len());
    let mut amort_days = Vec::new();
    let mut amort_fracs = Vec::new();
    for (d, pct) in amort {
        if d < maturity_day {
            amort_days.push(d);
            amort_fracs.push(pct / 100.0);
        }
    }
    let flows = Schedule::from_triples(&build_cash_flow(
        issue.nominal,
        period,
        &coupon_days,
        &rates_pct,
        &amort_days,
        &amort_fracs,
        0.0,
        OFFER_NONE,
        0.0,
    ));
    let coupon_amount =
        issue.nominal * rates_pct.first().copied().unwrap_or(0.0) / 100.0 * period / YEAR;
    let accrued = accrued_interest(coupon_amount, days_since_last, period);
    let dirty_price = issue.price_pct / 100.0 * issue.nominal + issue.accrued.unwrap_or(accrued);
    let amounts = flows.amounts();
    let ytm_maturity = ytm_effective(&amounts, &flows.days, dirty_price);
    let ytm_simple = ytm_simple(&amounts, &flows.days, dirty_price);
    let macaulay = macaulay_duration(&amounts, &flows.days, ytm_maturity);
    let modified = modified_duration(&amounts, &flows.days, ytm_maturity);

    let offer_day =
        offer_days
            .into_iter()
            .filter(|&d| d > 0.0)
            .fold(None, |best: Option<f64>, d| match best {
                Some(b) if b <= d => Some(b),
                _ => Some(d),
            });
    let (flows_to_offer, ytm_offer) = match offer_day {
        Some(od) => {
            let s = Schedule::from_triples(&build_cash_flow(
                issue.nominal,
                period,
                &coupon_days,
                &rates_pct,
                &amort_days,
                &amort_fracs,
                od,
                OFFER_REDEEM,
                0.0,
            ));
            let y = ytm_effective(&s.amounts(), &s.days, dirty_price);
            (Some(s), Some(y))
        }
        None => (None, None),
    };
    let (event, event_day) = match offer_day {
        Some(od) => (Event::Offer, od),
        None => (Event::Maturity, maturity_day),
    };
    Ok(Derived {
        maturity_day,
        coupon_days,
        rates_pct,
        amort_days,
        amort_fracs,
        days_since_last,
        coupon_amount,
        accrued,
        dirty_price,
        flows,
        flows_to_offer,
        offer_day,
        ytm_maturity,
        ytm_offer,
        ytm_simple,
        event,
        event_day,
        yield_event: ytm_offer.unwrap_or(ytm_maturity),
        macaulay,
        modified,
    })
}
