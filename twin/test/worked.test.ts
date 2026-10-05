// Worked examples: small issues whose results are computed by hand, with
// the arithmetic in the comments. The same examples run against the Rust
// crate in tests/worked.rs.
import { describe, expect, it } from "vitest";
import { calculate, hold_value } from "../src/index.js";
import type { Calculation, Issue, Market, Plan } from "../src/index.js";

const close = (got: number, want: number) => expect(Math.abs(got - want)).toBeLessThanOrEqual(1e-9 * Math.max(Math.abs(want), 1));

const market = (valuationDate: string): Market => ({ valuationDate, keyRatePct: 16 });

function ok(r: ReturnType<typeof calculate>): Calculation {
  if (!("ok" in r)) throw new Error(r.error);
  return r.ok;
}

// A two-year floater at key rate plus 2, annual coupons, bought at par on a
// coupon day: valuation 2026-01-01, coupons on day 365 (2027-01-01) and day
// 730 (2028-01-01), no accrued interest. Its flows are 180 and 1,180 per
// bond, so its yield is 18 percent (1,180 / 1.18 = 1,000).
const floater: Issue = {
  nominal: 1000,
  pricePct: 100,
  accrued: null,
  couponType: "floater",
  couponRatePct: 0,
  spreadPct: 2,
  periodDays: 365,
  maturity: "2028-01-01",
  offers: [],
  amortization: [],
};

// Ten bonds held for a year, no tax, no reinvestment.
const floaterPlan = (rateShiftPct: number): Plan => ({
  amount: 10_000,
  horizonDay: 365,
  reinvest: false,
  taxRegime: "iis_b",
  taxRatePct: 13,
  rateShiftPct,
});

describe("floaters", () => {
  it("sells a floater at the spread to the key rate, not by duration", () => {
    const c = ok(calculate(floater, market("2026-01-01"), floaterPlan(2)));
    // The plan: coupon 180 x 10 = 1,800; the day-730 flow sold on day 365 at
    // 18 percent: 1,180 / 1.18 = 1,000 x 10 = 10,000; commission 0.05
    // percent of 10,000 bought and 10,000 sold = 10. Total 11,790.
    close(c.plan.total, 11_790);
    // Key rate +2 by the horizon, in equal steps on each coupon up to the
    // first one after the horizon: 17 and then 18 percent, so the coupons
    // pay 19 and 20 percent (190 and 200). The spread to the key rate
    // stays: the day-730 flow is discounted at 20 percent, 1,200 / 1.2 =
    // 1,000. Coupons 1,900, sale 10,000, commission 10: total 11,890, 100
    // more than the plan.
    const e = c.earlyExit;
    close(e.result.coupons, 1_900);
    close(e.result.body, 10_000);
    close(e.result.total, 11_890);
    close(e.diff, 100);
  });

  it("keeps a floater at par in every key-rate scenario", () => {
    const s = ok(calculate(floater, market("2026-01-01"), floaterPlan(0))).floater!.scenarios;
    // -2 over four coupons: 15.5 then 15 percent; coupons 175 and 170; the
    // day-730 flow, 1,170, discounted at 17 percent: 1,000.
    close(s[0]!.breakdown.body, 10_000);
    close(s[0]!.breakdown.total, 11_740);
    close(s[1]!.breakdown.total, 11_790);
    // +2: 16.5 then 17 percent; coupons 185 and 190; 1,190 / 1.19 = 1,000.
    close(s[2]!.breakdown.body, 10_000);
    close(s[2]!.breakdown.total, 11_840);
  });
});

// Two years, 10 percent annual coupons, half the nominal repaid with the
// first coupon: valuation 2026-01-01, coupons on day 365 and day 730,
// bought at par with no accrued interest. Per bond the flows are 100 + 500
// = 600 and 50 + 500 = 550, so the yield is 10 percent: 600 / 1.1 + 550 /
// 1.21 = 1,000.
const amortising: Issue = {
  nominal: 1000,
  pricePct: 100,
  accrued: null,
  couponType: "fixed",
  couponRatePct: 10,
  spreadPct: 0,
  periodDays: 365,
  maturity: "2028-01-01",
  offers: [],
  amortization: [{ date: "2027-01-01", fractionPct: 50 }],
};

describe("amortisation", () => {
  it("reinvests returned principal with the coupons", () => {
    // Coupon 100 and principal 500 on day 365 at 10 percent for a year: 60.
    const hv = hold_value([365, 730], [100, 50], [500, 500], 730, 0.1, 0.1);
    expect(Array.from(hv).map((x) => Math.round(x * 1e9) / 1e9)).toEqual([150, 60, 500, 500, 0]);
  });

  it("earns about the yield on an amortising plan", () => {
    const plan: Plan = { amount: 10_000, horizonDay: 730, reinvest: true, taxRegime: "iis_b", taxRatePct: 13, rateShiftPct: 0 };
    const b = ok(calculate(amortising, market("2026-01-01"), plan)).plan;
    // Coupons 1,000 + 500; the 5,000 repaid on day 365 and the 1,000
    // coupon earn 10 percent for a year: 600. Redemption 5,000; commission
    // 5. Total 12,095; annual sqrt(1.2095) - 1 = 9.977 percent.
    close(b.coupons, 1_500);
    close(b.reinvest, 600);
    close(b.amort, 5_000);
    close(b.body, 5_000);
    close(b.total, 12_095);
    close(b.annualPct, 9.977_270_378_928_749);
  });
});
