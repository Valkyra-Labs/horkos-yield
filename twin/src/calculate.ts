/*
  calculate: an issue, its market and a plan in; the plan's totals, the
  early exit under a key-rate shift, floater scenarios and the offer pair
  out.
*/

import { addYears, civilFromDays, fullYears, parseIsoDate } from "./dates.js";
import { amountsOf, derive_bond, isCouponType, scheduleFromTriples } from "./issue.js";
import {
  OFFER_NONE,
  OFFER_RATE_CHANGE,
  YEAR,
  build_cash_flow,
  floater_rate_path,
  hold_value,
  income_tax,
  modified_duration,
  periodic_rate_pct,
  price_after_rate_shift,
  value_along_path,
} from "./primitives.js";
import type {
  Breakdown,
  Calculation,
  Derived,
  ErrorCode,
  FloaterScenario,
  Issue,
  Market,
  Plan,
  Result,
  Schedule,
  TaxRegime,
} from "./types.js";

export const COMMISSION_PCT = 0.05;
export const WORST_CASE_COUPON_PCT = 0.1;
export const FLOATER_SHIFTS_PCT: readonly number[] = [-2, 0, 2];
export const FLOATER_RAMP_STEPS = 4;
export const MAX_AMOUNT = 1e9;
/* Long-term holding relief: held more than this many years, at most the cap per full year held */
export const LDV_YEARS = 3;
export const LDV_CAP_PER_YEAR = 3_000_000;
/* The shortest horizon, in days, whose return is annualised */
export const MIN_ANNUALISED_DAYS = 30;

function isTaxRegime(code: unknown): code is TaxRegime {
  return code === "standard" || code === "iis_b";
}

/*
  Effective annual return in percent: the deposit rate that turns invested
  into total over horizonDay days. 0 when either is not positive, -100 when
  total is not positive.
*/
export function effective_annual_pct(invested: number, total: number, horizonDay: number): number {
  if (!(invested > 0) || !(horizonDay > 0)) return 0;
  if (total <= 0) return -100;
  return (Math.pow(total / invested, YEAR / horizonDay) - 1) * 100;
}

type Holding = {
  qty: number;
  dirtyPrice: number;
  /* Accrued interest paid per bond at purchase */
  accruedPaid: number;
  nominal: number;
  /* The valuation date as days since 1970-01-01 */
  today: number;
  periodDays: number;
  reinvestRate: number;
  exitYield: number;
  plan: Plan;
};

/*
  How the flows after the horizon are sold: at the exit yield moved by the
  modified duration for a key-rate shift (a fixed coupon), or discounted
  along a path of per-period rates (a floater).
*/
type Sale = { shiftPct: number } | { pathPct: readonly number[] };

function breakdownOf(
  h: Holding,
  flows: Schedule,
  horizonDay: number,
  how: Sale,
): { breakdown: Breakdown; modDurationAtHorizon: number | null } {
  const hv = hold_value(flows.days, flows.coupons, flows.principals, horizonDay, h.reinvestRate, h.exitYield);
  let sale = hv[4] as number;
  let modDurationAtHorizon: number | null = null;
  if ("shiftPct" in how) {
    modDurationAtHorizon = 0;
    if (sale > 0) {
      const amounts: number[] = [];
      const shifted: number[] = [];
      flows.days.forEach((d, i) => {
        if (d > horizonDay) {
          amounts.push((flows.coupons[i] as number) + (flows.principals[i] as number));
          shifted.push(d - horizonDay);
        }
      });
      modDurationAtHorizon = modified_duration(amounts, shifted, h.exitYield);
      if (how.shiftPct !== 0) sale = price_after_rate_shift(sale, modDurationAtHorizon, how.shiftPct);
    }
  } else if (sale > 0) {
    sale = value_along_path(flows.days, amountsOf(flows), horizonDay, h.periodDays, how.pathPct);
  }
  const { qty } = h;
  const invested = qty * h.dirtyPrice;
  const coupons = (hv[0] as number) * qty;
  const reinvest = (hv[1] as number) * qty;
  const amort = (hv[2] as number) * qty;
  const body = ((hv[3] as number) + sale) * qty;
  const sold = sale > 0 ? sale * qty : 0;
  const commission = ((invested + sold) * COMMISSION_PCT) / 100;
  const tax = taxOf(h, flows, horizonDay, reinvest, invested, sold);
  const total = coupons + reinvest + amort + body - tax - commission;
  return {
    breakdown: {
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
      periodPct: ((total - invested) / invested) * 100,
      annualPct: horizonDay >= MIN_ANNUALISED_DAYS ? effective_annual_pct(invested, total, horizonDay) : null,
      horizonDay,
    },
    modDurationAtHorizon,
  };
}

/*
  One calendar year of the tax base; relieved is the result of disposals
  under the long-term holding relief, with what they returned and that
  weighted by the full years each was held
*/
type TaxYear = { year: number; income: number; result: number; relieved: number; relievedProceeds: number; relievedYears: number };

/*
  Personal income tax summed over the calendar years the position pays in.
  Each year's coupons and the result of redemptions and the sale form one
  base, taxed at zero when negative and not carried to another year.
  Accrued interest paid at purchase reduces the first coupon received (up
  to that coupon) and the cost; the cost with the purchase commission is
  spread over redemptions and the sale by the nominal each returns; the
  sale bears its own commission; reinvestment income falls in the
  horizon's year. A redemption or sale more than three years after the
  purchase by calendar anniversary is relieved: the year's positive relieved
  result is exempt up to 3 million times the full years held, averaged over
  the relieved disposals weighted by what each returned; coupons stay taxed.
*/
function taxOf(h: Holding, flows: Schedule, horizonDay: number, reinvest: number, invested: number, sold: number): number {
  if (h.plan.taxRegime === "iis_b") return 0;
  const { qty } = h;
  const yearOf = (day: number) => civilFromDays(h.today + Math.floor(day))[0];
  const reliefAfter = addYears(h.today, LDV_YEARS) - h.today;
  const held = (day: number): number | null =>
    Math.floor(day) > reliefAfter ? fullYears(h.today, h.today + Math.floor(day)) : null;
  const years: TaxYear[] = [];
  const entry = (year: number): TaxYear => {
    let t = years.find((x) => x.year === year);
    if (!t) {
      t = { year, income: 0, result: 0, relieved: 0, relievedProceeds: 0, relievedYears: 0 };
      years.push(t);
    }
    return t;
  };
  const book = (t: TaxYear, result: number, proceeds: number, years: number | null) => {
    if (years === null) {
      t.result += result;
    } else {
      t.relieved += result;
      t.relievedProceeds += proceeds;
      t.relievedYears += years * proceeds;
    }
  };
  const firstDay = flows.days[0];
  const firstCoupon = firstDay !== undefined && firstDay <= horizonDay ? (flows.coupons[0] as number) * qty : 0;
  const deducted = Math.min(h.accruedPaid * qty, firstCoupon);
  const cost = invested + (invested * COMMISSION_PCT) / 100 - deducted;
  let repaid = 0;
  for (let i = 0; i < flows.days.length; i++) {
    const d = flows.days[i] as number;
    if (d > horizonDay) break;
    const t = entry(yearOf(d));
    t.income += (flows.coupons[i] as number) * qty - (i === 0 ? deducted : 0);
    const p = flows.principals[i] as number;
    if (p > 0) {
      book(t, p * qty - (cost * p) / h.nominal, p * qty, held(d));
      repaid += p;
    }
  }
  const t = entry(yearOf(horizonDay));
  if (sold > 0) book(t, sold - (sold * COMMISSION_PCT) / 100 - cost * ((h.nominal - repaid) / h.nominal), sold, held(horizonDay));
  t.income += reinvest;
  let tax = 0;
  for (const y of years) {
    const exempt = y.relieved > 0 ? Math.min(y.relieved, (LDV_CAP_PER_YEAR * y.relievedYears) / y.relievedProceeds) : 0;
    const base = y.income + y.result + y.relieved - exempt;
    tax += income_tax(base, h.plan.otherIncome);
  }
  return tax;
}

/*
  A floater's flows when the key rate moves by shiftPct in equal steps on
  each of the first `steps` coupons, and the per-period rates to discount
  them at: today's rate for the issue moved by the same change of the key
  rate, so the spread the market asks over the key rate stays.
*/
function floaterPath(issue: Issue, market: Market, d: Derived, shiftPct: number, steps: number): { flows: Schedule; discount: number[] } {
  const key = Array.from(floater_rate_path(market.keyRatePct, shiftPct, steps, d.couponDays.length));
  const flows = scheduleFromTriples(
    build_cash_flow(
      issue.nominal,
      issue.periodDays,
      d.couponDays,
      key.map((k) => k + issue.spreadPct),
      d.amortDays,
      d.amortFracs,
      0,
      OFFER_NONE,
      0,
    ),
  );
  const today = periodic_rate_pct(d.ytmMaturity, issue.periodDays);
  return { flows, discount: key.map((k) => today + (k - market.keyRatePct)) };
}

/* Bonds bought, or the first plan error in the documented order */
function checkPlan(d: Derived, plan: Plan): number | ErrorCode {
  if (!Number.isFinite(plan.amount) || plan.amount <= 0) return "amount_not_positive";
  if (plan.amount > MAX_AMOUNT) return "amount_too_large";
  if (!Number.isFinite(plan.horizonDay) || plan.horizonDay < 1 || plan.horizonDay > d.maturityDay) {
    return "horizon_out_of_range";
  }
  if (!(Number.isFinite(plan.otherIncome) && plan.otherIncome >= 0)) return "invalid_other_income";
  if (!(Number.isFinite(d.dirtyPrice) && d.dirtyPrice > 0)) return "invalid_price";
  const qty = Math.floor(plan.amount / d.dirtyPrice);
  return qty < 1 ? "amount_below_one_bond" : qty;
}

export function calculate(issue: Issue, market: Market, plan: Plan): Result<Calculation> {
  if (!isCouponType(issue.couponType) || !isTaxRegime(plan.taxRegime)) {
    return { error: "invalid_code" };
  }
  const derived = derive_bond(issue, market);
  if ("error" in derived) return derived;
  const d = derived.ok;
  const qty = checkPlan(d, plan);
  if (typeof qty === "string") return { error: qty };
  const y = d.ytmMaturity;
  const h: Holding = {
    qty,
    dirtyPrice: d.dirtyPrice,
    accruedPaid: issue.accrued ?? d.accrued,
    nominal: issue.nominal,
    today: parseIsoDate(market.valuationDate) as number,
    periodDays: issue.periodDays,
    reinvestRate: plan.reinvest ? y : 0,
    exitYield: y,
    plan,
  };

  const base = breakdownOf(h, d.flows, plan.horizonDay, { shiftPct: 0 }).breakdown;
  const applicable = plan.horizonDay < d.maturityDay;
  const shift = applicable ? plan.rateShiftPct : 0;
  let early: ReturnType<typeof breakdownOf>;
  if (issue.couponType === "floater" && shift === 0) {
    // An unchanged key rate is the plan itself.
    early = { breakdown: base, modDurationAtHorizon: null };
  } else if (issue.couponType === "floater") {
    const paid = d.couponDays.filter((day) => day <= plan.horizonDay).length;
    const { flows, discount } = floaterPath(issue, market, d, shift, paid + 1);
    early = breakdownOf(h, flows, plan.horizonDay, { pathPct: discount });
  } else {
    early = breakdownOf(h, d.flows, plan.horizonDay, { shiftPct: shift });
  }

  let floater: Calculation["floater"] = null;
  if (issue.couponType === "floater") {
    const scenarios = FLOATER_SHIFTS_PCT.map((shiftPct): FloaterScenario => {
      const { flows, discount } = floaterPath(issue, market, d, shiftPct, FLOATER_RAMP_STEPS);
      // An unchanged key rate is the plan itself.
      const how: Sale = shiftPct === 0 ? { shiftPct: 0 } : { pathPct: discount };
      return { shiftPct, breakdown: breakdownOf(h, flows, plan.horizonDay, how).breakdown, coupons: flows.coupons };
    });
    floater = { days: [...d.couponDays], scenarios };
  }

  let offer: Calculation["offer"] = null;
  if (d.offerDay !== null && d.flowsToOffer !== null) {
    const before = breakdownOf(h, d.flowsToOffer, d.offerDay, { shiftPct: 0 }).breakdown;
    const worst = scheduleFromTriples(
      build_cash_flow(
        issue.nominal,
        issue.periodDays,
        d.couponDays,
        d.ratesPct,
        d.amortDays,
        d.amortFracs,
        d.offerDay,
        OFFER_RATE_CHANGE,
        WORST_CASE_COUPON_PCT,
      ),
    );
    offer = { before, after: breakdownOf(h, worst, d.maturityDay, { shiftPct: 0 }).breakdown };
  }

  return {
    ok: {
      plan: base,
      earlyExit: {
        rateShiftPct: plan.rateShiftPct,
        applicable,
        result: early.breakdown,
        diff: early.breakdown.total - base.total,
        modDurationAtHorizon: early.modDurationAtHorizon,
      },
      floater,
      offer,
    },
  };
}
