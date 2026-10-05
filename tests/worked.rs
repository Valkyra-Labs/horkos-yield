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
