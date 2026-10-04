/*
  derive_bond: an issue and its market in; schedule, flows to maturity and
  to the nearest offer, accrued interest, yields and durations out.
*/

import { parseIsoDate } from "./dates.js";
import {
  OFFER_NONE,
  OFFER_REDEEM,
  YEAR,
  accrued_interest,
  build_cash_flow,
  macaulay_duration,
  modified_duration,
  ytm_effective,
  ytm_simple,
} from "./primitives.js";
import type { Derived, Issue, Market, Result, Schedule } from "./types.js";

export function scheduleFromTriples(triples: ArrayLike<number>): Schedule {
  const s: Schedule = { days: [], coupons: [], principals: [] };
  for (let i = 0; i + 2 < triples.length; i += 3) {
    s.days.push(triples[i] as number);
    s.coupons.push(triples[i + 1] as number);
    s.principals.push(triples[i + 2] as number);
  }
  return s;
}

export function amountsOf(s: Schedule): number[] {
  return s.coupons.map((c, i) => c + (s.principals[i] as number));
}

/* Coupon days counted back from maturity while after the valuation date, ascending */
export function coupon_schedule(maturityDay: number, periodDays: number): number[] {
  const days: number[] = [];
  for (let d = maturityDay; d > 0; d -= periodDays) days.push(d);
  return days.reverse();
}

export function isCouponType(code: unknown): boolean {
  return code === "fixed" || code === "floater";
}

export function baseRate(issue: Issue, market: Market): number {
  return issue.couponType === "floater"
    ? market.keyRatePct + issue.spreadPct
    : issue.couponRatePct;
}

/*
  Errors in order: invalid_code (coupon type), invalid_date (valuation,
  maturity, offers, amortisation), invalid_nominal, invalid_period, matured.
  A price that is not positive gives NaN yields, not an error.
*/
export function derive_bond(issue: Issue, market: Market): Result<Derived> {
  if (!isCouponType(issue.couponType)) return { error: "invalid_code" };
  const today = parseIsoDate(market.valuationDate);
  if (today === null) return { error: "invalid_date" };
  const dayOf = (iso: string): number | null => {
    const d = parseIsoDate(iso);
    return d === null ? null : d - today;
  };
  const maturityDay = dayOf(issue.maturity);
  if (maturityDay === null) return { error: "invalid_date" };
  const offerDays: number[] = [];
  for (const o of issue.offers) {
    const d = dayOf(o);
    if (d === null) return { error: "invalid_date" };
    offerDays.push(d);
  }
  const amort: [number, number][] = [];
  for (const a of issue.amortization) {
    const d = dayOf(a.date);
    if (d === null) return { error: "invalid_date" };
    amort.push([d, a.fractionPct]);
  }
  if (!(Number.isFinite(issue.nominal) && issue.nominal > 0)) return { error: "invalid_nominal" };
  if (!(Number.isFinite(issue.periodDays) && issue.periodDays >= 1)) {
    return { error: "invalid_period" };
  }
  if (maturityDay <= 0) return { error: "matured" };

  const period = issue.periodDays;
  const couponDays = coupon_schedule(maturityDay, period);
  const daysSinceLast = period - (couponDays[0] ?? maturityDay);
  const rate = baseRate(issue, market);
  const ratesPct = couponDays.map(() => rate);
  const amortDays: number[] = [];
  const amortFracs: number[] = [];
  for (const [d, pct] of amort) {
    if (d < maturityDay) {
      amortDays.push(d);
      amortFracs.push(pct / 100);
    }
  }
  const flows = scheduleFromTriples(
    build_cash_flow(issue.nominal, period, couponDays, ratesPct, amortDays, amortFracs, 0, OFFER_NONE, 0),
  );
  const couponAmount = (((issue.nominal * (ratesPct[0] ?? 0)) / 100) * period) / YEAR;
  const accrued = accrued_interest(couponAmount, daysSinceLast, period);
  const dirtyPrice = (issue.pricePct / 100) * issue.nominal + (issue.accrued ?? accrued);
  const amounts = amountsOf(flows);
  const ytmMaturity = ytm_effective(amounts, flows.days, dirtyPrice);

  let offerDay: number | null = null;
  for (const d of offerDays) if (d > 0 && (offerDay === null || d < offerDay)) offerDay = d;
  let flowsToOffer: Schedule | null = null;
  let ytmOffer: number | null = null;
  if (offerDay !== null) {
    flowsToOffer = scheduleFromTriples(
      build_cash_flow(
        issue.nominal,
        period,
        couponDays,
        ratesPct,
        amortDays,
        amortFracs,
        offerDay,
        OFFER_REDEEM,
        0,
      ),
    );
    ytmOffer = ytm_effective(amountsOf(flowsToOffer), flowsToOffer.days, dirtyPrice);
  }
  return {
    ok: {
      maturityDay,
      couponDays,
      ratesPct,
      amortDays,
      amortFracs,
      daysSinceLast,
      couponAmount,
      accrued,
      dirtyPrice,
      flows,
      flowsToOffer,
      offerDay,
      ytmMaturity,
      ytmOffer,
      ytmSimple: ytm_simple(amounts, flows.days, dirtyPrice),
      event: offerDay === null ? "maturity" : "offer",
      eventDay: offerDay ?? maturityDay,
      yieldEvent: ytmOffer ?? ytmMaturity,
      macaulay: macaulay_duration(amounts, flows.days, ytmMaturity),
      modified: modified_duration(amounts, flows.days, ytmMaturity),
    },
  };
}
