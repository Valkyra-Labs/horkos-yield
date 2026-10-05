//! Worked examples: small issues whose results are computed by hand, with
//! the arithmetic in the comments. The same examples run against the
//! TypeScript twin in `twin/test/worked.test.ts`.

use horkos_yield::*;

fn close(got: f64, want: f64) -> bool {
    (got - want).abs() <= 1e-9 * want.abs().max(1.0)
}

macro_rules! assert_close {
    ($got:expr, $want:expr) => {{
        let (g, w) = ($got, $want);
        assert!(close(g, w), "{} = {g}, want {w}", stringify!($got));
    }};
}

fn market(valuation_date: &str) -> Market {
    Market {
        valuation_date: valuation_date.into(),
        key_rate_pct: 16.0,
    }
}

// A two-year floater at key rate plus 2, annual coupons, bought at par on
// a coupon day: valuation 2026-01-01, coupons on day 365 (2027-01-01) and
// day 730 (2028-01-01), no accrued interest. Its flows are 180 and 1,180
// per bond, so its yield is 18 percent (1,180 / 1.18 = 1,000).
fn floater() -> Issue {
    Issue {
        nominal: 1000.0,
        price_pct: 100.0,
        accrued: None,
        coupon_type: CouponType::Floater,
        coupon_rate_pct: 0.0,
        spread_pct: 2.0,
        period_days: 365.0,
        maturity: "2028-01-01".into(),
        offers: vec![],
        amortization: vec![],
    }
}

// Ten bonds held for a year, no tax, no reinvestment.
fn floater_plan(rate_shift_pct: f64) -> Plan {
    Plan {
        amount: 10_000.0,
        horizon_day: 365.0,
        reinvest: false,
        tax_regime: TaxRegime::IisB,
        tax_rate_pct: 13.0,
        rate_shift_pct,
    }
}

#[test]
fn floater_sale_follows_the_key_rate_not_duration() {
    let c = calculate(&floater(), &market("2026-01-01"), &floater_plan(2.0)).unwrap();
    // The plan: coupon 180 x 10 = 1,800; the day-730 flow sold on day 365
    // at 18 percent: 1,180 / 1.18 = 1,000 x 10 = 10,000; commission 0.05
    // percent of 10,000 bought and 10,000 sold = 10.
    // Total 1,800 + 10,000 - 10 = 11,790.
    assert_close!(c.plan.total, 11_790.0);
    // Key rate +2 by the horizon: it moves in equal steps on each coupon up
    // to the first one after the horizon, 16 + 1 = 17 and then 18 percent,
    // so the coupons are 17 + 2 = 19 percent (190) and 18 + 2 = 20 percent
    // (200). The spread to the key rate stays: the day-730 flow is
    // discounted at 18 + 2 = 20 percent, 1,200 / 1.2 = 1,000. Coupons
    // 1,900, sale 10,000, commission 10: total 11,890, 100 more than the
    // plan. A fixed bond's duration (1 / 1.18 = 0.847) would have taken
    // 1.7 percent off the price instead.
    let e = c.early_exit;
    assert_close!(e.result.coupons, 1_900.0);
    assert_close!(e.result.body, 10_000.0);
    assert_close!(e.result.total, 11_890.0);
    assert_close!(e.diff, 100.0);
}

#[test]
fn floater_scenarios_keep_the_price_at_par() {
    let c = calculate(&floater(), &market("2026-01-01"), &floater_plan(0.0)).unwrap();
    let s = c.floater.unwrap().scenarios;
    // Key rate -2 over four coupons: 16 - 0.5 = 15.5, then 15 percent; the
    // coupons pay 17.5 and 17 percent (175 and 170). The day-730 flow, 1,170,
    // is discounted at 18 - 1 = 17 percent: 1,000. Total 1,750 + 10,000 - 10.
    assert_close!(s[0].breakdown.body, 10_000.0);
    assert_close!(s[0].breakdown.total, 11_740.0);
    // Unchanged: 1,800 + 10,000 - 10.
    assert_close!(s[1].breakdown.total, 11_790.0);
    // Key rate +2: 16.5 then 17 percent, coupons 185 and 190; 1,190 / 1.19
    // = 1,000. Total 1,850 + 10,000 - 10.
    assert_close!(s[2].breakdown.body, 10_000.0);
    assert_close!(s[2].breakdown.total, 11_840.0);
}

// Two years, 10 percent annual coupons, half the nominal repaid with the
// first coupon: valuation 2026-01-01, coupons on day 365 (2027-01-01) and
// day 730 (2028-01-01), bought at par with no accrued interest. Per bond
// the flows are 100 + 500 = 600 and then 10 percent of the 500 left, 50 +
// 500 = 550, so the yield is 10 percent: 600 / 1.1 + 550 / 1.21 = 1,000.
fn amortising() -> Issue {
    Issue {
        nominal: 1000.0,
        price_pct: 100.0,
        accrued: None,
        coupon_type: CouponType::Fixed,
        coupon_rate_pct: 10.0,
        spread_pct: 0.0,
        period_days: 365.0,
        maturity: "2028-01-01".into(),
        offers: vec![],
        amortization: vec![Amortization {
            date: "2027-01-01".into(),
            fraction_pct: 50.0,
        }],
    }
}

#[test]
fn hold_value_reinvests_returned_principal() {
    // Coupon 100 and principal 500 on day 365, reinvested at 10 percent for
    // the year to day 730: 600 x 0.1 = 60.
    let hv = hold_value(
        &[365.0, 730.0],
        &[100.0, 50.0],
        &[500.0, 500.0],
        730.0,
        0.1,
        0.1,
    );
    assert_close!(hv[0], 150.0);
    assert_close!(hv[1], 60.0);
    assert_close!(hv[2], 500.0);
    assert_close!(hv[3], 500.0);
    assert_close!(hv[4], 0.0);
}

#[test]
fn amortising_plan_earns_about_its_yield() {
    let plan = Plan {
        amount: 10_000.0,
        horizon_day: 730.0,
        reinvest: true,
        tax_regime: TaxRegime::IisB,
        tax_rate_pct: 13.0,
        rate_shift_pct: 0.0,
    };
    let b = calculate(&amortising(), &market("2026-01-01"), &plan)
        .unwrap()
        .plan;
    // Ten bonds for 10,000. Coupons 1,000 + 500; the 5,000 repaid on day
    // 365 and the 1,000 coupon both earn 10 percent for a year: 600.
    // Final redemption 5,000; commission 0.05 percent of 10,000 = 5.
    // Total 1,500 + 600 + 5,000 + 5,000 - 5 = 12,095, and the effective
    // annual return is sqrt(1.2095) - 1 = 9.977 percent: the yield, less the
    // commission. With the repaid principal left idle it was
    // sqrt(1.1595) - 1 = 7.68 percent.
    assert_close!(b.coupons, 1_500.0);
    assert_close!(b.reinvest, 600.0);
    assert_close!(b.amort, 5_000.0);
    assert_close!(b.body, 5_000.0);
    assert_close!(b.total, 12_095.0);
    assert_close!(b.annual_pct.unwrap(), 9.977_270_378_928_749);
}

#[test]
fn no_annual_return_under_a_month() {
    // The par floater above for 29 and for 30 days. Under 30 days the
    // return is given over the period only; compounded to a year, the
    // 0.1 percent of commission alone would read as about -1.2 percent a
    // year at 30 days and -30 percent at one day.
    let at = |horizon_day: f64| {
        let plan = Plan {
            horizon_day,
            ..floater_plan(0.0)
        };
        calculate(&floater(), &market("2026-01-01"), &plan)
            .unwrap()
            .plan
    };
    let short = at(29.0);
    assert_eq!(short.annual_pct, None);
    assert_close!(
        short.period_pct,
        (short.total / short.invested - 1.0) * 100.0
    );
    let month = at(30.0);
    assert_eq!(
        month.annual_pct,
        Some(effective_annual_pct(month.invested, month.total, 30.0))
    );
    assert_eq!(MIN_ANNUALISED_DAYS, 30.0);
}

// Annual 10 percent coupons, bought above par between coupons: valuation
// 2026-01-01, maturity 2027-07-02 (day 547), so the coupons fall on day 182
// (2026-07-02) and day 547 (2027-07-02). 183 days of the 365-day period
// have passed: accrued interest 100 x 183 / 365 = 50.136986. Clean price
// 1,050, dirty 1,100.136986.
fn above_par() -> Issue {
    Issue {
        nominal: 1000.0,
        price_pct: 105.0,
        accrued: None,
        coupon_type: CouponType::Fixed,
        coupon_rate_pct: 10.0,
        spread_pct: 0.0,
        period_days: 365.0,
        maturity: "2027-07-02".into(),
        offers: vec![],
        amortization: vec![],
    }
}

#[test]
fn tax_nets_accrued_interest_and_the_loss_against_coupons() {
    let plan = Plan {
        amount: 11_100.0,
        horizon_day: 547.0,
        reinvest: false,
        tax_regime: TaxRegime::Standard,
        tax_rate_pct: 13.0,
        rate_shift_pct: 0.0,
    };
    let b = calculate(&above_par(), &market("2026-01-01"), &plan)
        .unwrap()
        .plan;
    // Ten bonds: invested 11,001.369863, of which accrued interest
    // 501.369863; commission on the purchase 0.05 percent, 5.500685.
    // 2026: the first coupon, 1,000, less the accrued interest paid for it,
    // 501.369863: 498.630137 taxable.
    // 2027: the second coupon, 1,000, and the redemption: 10,000 received
    // against a cost of 11,001.369863 - 501.369863 (already deducted from
    // the coupon) + 5.500685 commission = 10,505.500685, a loss of
    // 505.500685, so 1,000 - 505.500685 = 494.499315 taxable.
    // Tax 13 percent of 498.630137 + 494.499315 = 129.106829 (it was 13
    // percent of both coupons, 260). Total 2,000 + 10,000 - 129.106829 -
    // 5.500685 = 11,865.392486.
    assert_close!(b.invested, 11_001.369_863_013_699);
    assert_close!(b.tax, -129.106_828_767_123_3);
    assert_close!(b.total, 11_865.392_486_301_369);
}
