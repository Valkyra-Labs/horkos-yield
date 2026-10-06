/*
  Primitives on flat number arrays, the same contract as the Rust crate's
  primitives module. Days are offsets from the valuation date, ACT/365;
  rates are annual effective fractions unless the name ends in Pct.
*/

import type { Num } from "./types.js";

export const YEAR = 365;

export const OFFER_NONE = 0;
export const OFFER_REDEEM = 1;
export const OFFER_RATE_CHANGE = 2;

/* Income tax: 13 percent up to 2.4 million of a year's investment income, 15 above */
export const TAX_RATE_PCT = 13;
export const TAX_HIGHER_RATE_PCT = 15;
export const TAX_THRESHOLD = 2_400_000;

const at = (xs: Num, i: number): number => xs[i] as number;

export function price_from_yield(amounts: Num, days: Num, y: number): number {
  const base = 1 + y;
  const n = Math.min(amounts.length, days.length);
  let pv = 0;
  for (let i = 0; i < n; i++) pv += at(amounts, i) * Math.pow(base, -at(days, i) / YEAR);
  return pv;
}

/* Bisection on [-0.99, 10] with 200 steps; NaN without flows or a positive price */
export function ytm_effective(amounts: Num, days: Num, price: number): number {
  if (amounts.length === 0 || !(price > 0)) return Number.NaN;
  let lo = -0.99;
  let hi = 10;
  for (let i = 0; i < 200; i++) {
    const mid = 0.5 * (lo + hi);
    if (price_from_yield(amounts, days, mid) > price) lo = mid;
    else hi = mid;
  }
  return 0.5 * (lo + hi);
}

/*
  Simple yield over the full term: all flows less the price, over the price,
  divided by the years to the last flow; not compounded, so below the yield
  to maturity for an amortising issue.
*/
export function ytm_simple(amounts: Num, days: Num, price: number): number {
  if (amounts.length === 0 || !(price > 0)) return Number.NaN;
  let total = 0;
  for (let i = 0; i < amounts.length; i++) total += at(amounts, i);
  let last = 0;
  for (let i = 0; i < days.length; i++) last = Math.max(last, at(days, i));
  if (last <= 0) return Number.NaN;
  return ((total - price) / price) * (YEAR / last);
}

export function accrued_interest(coupon: number, daysSinceLast: number, periodDays: number): number {
  if (periodDays <= 0) return 0;
  return (coupon * Math.max(daysSinceLast, 0)) / periodDays;
}

export function macaulay_duration(amounts: Num, days: Num, y: number): number {
  const base = 1 + y;
  const n = Math.min(amounts.length, days.length);
  let pvSum = 0;
  let weighted = 0;
  for (let i = 0; i < n; i++) {
    const t = at(days, i) / YEAR;
    const pv = at(amounts, i) * Math.pow(base, -t);
    pvSum += pv;
    weighted += t * pv;
  }
  if (pvSum === 0) return 0;
  return weighted / pvSum;
}

export function modified_duration(amounts: Num, days: Num, y: number): number {
  return macaulay_duration(amounts, days, y) / (1 + y);
}

/* Flat triples [day, coupon, principal] */
export function build_cash_flow(
  nominal: number,
  periodDays: number,
  couponDays: Num,
  couponRatesPct: Num,
  amortDays: Num,
  amortFracs: Num,
  offerDay: number,
  offerMode: number,
  postOfferRatePct: number,
): Float64Array {
  const n = couponDays.length;
  const out: number[] = [];
  let outstanding = nominal;
  const hasOffer = offerDay > 0;
  const m = Math.min(amortDays.length, amortFracs.length);
  for (let i = 0; i < n; i++) {
    const d = at(couponDays, i);
    let rate = i < couponRatesPct.length ? at(couponRatesPct, i) : 0;
    if (offerMode === OFFER_RATE_CHANGE && hasOffer && d > offerDay) rate = postOfferRatePct;
    const coupon = (((outstanding * rate) / 100) * periodDays) / YEAR;
    const isLast = i + 1 === n;
    const isOffer = offerMode === OFFER_REDEEM && hasOffer && d >= offerDay;
    let principal: number;
    if (isLast || isOffer) {
      principal = outstanding;
    } else {
      let scheduled = 0;
      for (let j = 0; j < m; j++) {
        if (Math.abs(at(amortDays, j) - d) < 0.5) scheduled += nominal * at(amortFracs, j);
      }
      principal = Math.min(scheduled, outstanding);
    }
    out.push(d, coupon, principal);
    outstanding -= principal;
    if (isLast || isOffer) break;
  }
  return Float64Array.from(out);
}

export function floater_rate_path(
  basePct: number,
  deltaPct: number,
  rampSteps: number,
  n: number,
): Float64Array {
  const ramp = Math.max(rampSteps, 1);
  const out = new Float64Array(n);
  for (let i = 0; i < n; i++) out[i] = basePct + (deltaPct * Math.min(i + 1, ramp)) / ramp;
  return out;
}

export function floater_coupons(
  nominal: number,
  spreadPct: number,
  keyRatesPct: Num,
  periodDays: number,
): Float64Array {
  const out = new Float64Array(keyRatesPct.length);
  for (let i = 0; i < keyRatesPct.length; i++) {
    out[i] = (((nominal * (at(keyRatesPct, i) + spreadPct)) / 100) * periodDays) / YEAR;
  }
  return out;
}

/*
  Tax on one year's taxable base when the holder's other investment income
  that year is otherIncome: 13 percent on the part that stays within the
  threshold with the other income, 15 on the rest; nothing on a base that
  is not positive.
*/
export function income_tax(base: number, otherIncome: number): number {
  const b = Math.max(base, 0);
  const low = Math.min(Math.max(TAX_THRESHOLD - otherIncome, 0), b);
  return (low * TAX_RATE_PCT) / 100 + ((b - low) * TAX_HIGHER_RATE_PCT) / 100;
}

/*
  [coupons, reinvest income, amortisation, final principal, sale price] per
  bond; coupons and principal paid by the horizon are reinvested at
  reinvestRate when it is positive.
*/
export function hold_value(
  days: Num,
  coupons: Num,
  principals: Num,
  horizonDay: number,
  reinvestRate: number,
  exitYield: number,
): Float64Array {
  let last = Number.NEGATIVE_INFINITY;
  for (let i = 0; i < days.length; i++) last = Math.max(last, at(days, i));
  let couponsSum = 0;
  let reinvest = 0;
  let amort = 0;
  let fin = 0;
  let sale = 0;
  for (let i = 0; i < days.length; i++) {
    const d = at(days, i);
    const c = i < coupons.length ? at(coupons, i) : 0;
    const p = i < principals.length ? at(principals, i) : 0;
    if (d <= horizonDay) {
      couponsSum += c;
      if (reinvestRate > 0) {
        reinvest += (c + p) * (Math.pow(1 + reinvestRate, (horizonDay - d) / YEAR) - 1);
      }
      if (Math.abs(d - last) < 0.5) fin += p;
      else amort += p;
    } else {
      sale += (c + p) * Math.pow(1 + exitYield, -(d - horizonDay) / YEAR);
    }
  }
  return Float64Array.from([couponsSum, reinvest, amort, fin, sale]);
}

/* The rate in percent compounded once a period that equals the annual effective yield y */
export function periodic_rate_pct(y: number, periodDays: number): number {
  return ((Math.pow(1 + y, periodDays / YEAR) - 1) * YEAR) / periodDays * 100;
}

/*
  Value at horizonDay of the flows after it, discounted period by period at
  ratesPct[i] (compounded once a period) over the period that ends on
  days[i], the first one only for the part left after the horizon. A
  missing rate is zero.
*/
export function value_along_path(days: Num, amounts: Num, horizonDay: number, periodDays: number, ratesPct: Num): number {
  let factor = 1;
  let from = horizonDay;
  let pv = 0;
  for (let i = 0; i < days.length; i++) {
    const d = at(days, i);
    if (d <= horizonDay) continue;
    const rate = i < ratesPct.length ? at(ratesPct, i) : 0;
    factor *= Math.pow(1 + ((rate / 100) * periodDays) / YEAR, -(d - from) / periodDays);
    from = d;
    pv += (i < amounts.length ? at(amounts, i) : 0) * factor;
  }
  return pv;
}

export function price_after_rate_shift(price: number, modDuration: number, deltaPct: number): number {
  return Math.max(price * (1 - (modDuration * deltaPct) / 100), 0);
}
