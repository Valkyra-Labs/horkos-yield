// Loads the wasm-pack build in pkg/ under Node and wraps derive_bond and
// calculate so they take and return the same plain objects as the twin.
// Also the reference for using the structs from JavaScript.

import { readFileSync } from "node:fs";

const PKG = new URL("../pkg/", import.meta.url);

export async function loadWasm() {
  const glue = await import(new URL("horkos_yield.js", PKG).href);
  glue.initSync({ module: readFileSync(new URL("horkos_yield_bg.wasm", PKG)) });
  return glue;
}

// The wasm build behind the twin's API: plain objects in, plain objects
// out (arrays come back as Float64Array), every wasm object freed.
export function wrap(w) {
  const issueOf = (x) => {
    const i = new w.Issue();
    i.nominal = x.nominal;
    i.pricePct = x.pricePct;
    i.accrued = x.accrued ?? undefined;
    i.couponType = x.couponType;
    i.couponRatePct = x.couponRatePct;
    i.spreadPct = x.spreadPct;
    i.periodDays = x.periodDays;
    i.maturity = x.maturity;
    i.offers = x.offers;
    i.amortDates = x.amortization.map((a) => a.date);
    i.amortFractionsPct = Float64Array.from(x.amortization, (a) => a.fractionPct);
    return i;
  };
  const marketOf = (x) => {
    const m = new w.Market();
    m.valuationDate = x.valuationDate;
    m.keyRatePct = x.keyRatePct;
    return m;
  };
  const planOf = (x) => {
    const p = new w.Plan();
    p.amount = x.amount;
    p.horizonDay = x.horizonDay;
    p.reinvest = x.reinvest;
    p.taxRegime = x.taxRegime;
    p.taxRatePct = x.taxRatePct;
    p.rateShiftPct = x.rateShiftPct;
    return p;
  };
  const schedule = (s) => {
    const out = { days: s.days, coupons: s.coupons, principals: s.principals };
    s.free();
    return out;
  };
  const breakdown = (b) => {
    const out = {
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
      annualPct: b.annualPct,
      horizonDay: b.horizonDay,
    };
    b.free();
    return out;
  };
  const derived = (d) => {
    const toOffer = d.flowsToOffer;
    const out = {
      maturityDay: d.maturityDay,
      couponDays: d.couponDays,
      ratesPct: d.ratesPct,
      amortDays: d.amortDays,
      amortFracs: d.amortFracs,
      daysSinceLast: d.daysSinceLast,
      couponAmount: d.couponAmount,
      accrued: d.accrued,
      dirtyPrice: d.dirtyPrice,
      flows: schedule(d.flows),
      flowsToOffer: toOffer === undefined ? null : schedule(toOffer),
      offerDay: d.offerDay ?? null,
      ytmMaturity: d.ytmMaturity,
      ytmOffer: d.ytmOffer ?? null,
      ytmSimple: d.ytmSimple,
      event: d.event,
      eventDay: d.eventDay,
      yieldEvent: d.yieldEvent,
      macaulay: d.macaulay,
      modified: d.modified,
    };
    d.free();
    return out;
  };
  const calculation = (c) => {
    const e = c.earlyExit;
    const f = c.floater;
    const o = c.offer;
    const out = {
      plan: breakdown(c.plan),
      earlyExit: {
        rateShiftPct: e.rateShiftPct,
        applicable: e.applicable,
        result: breakdown(e.result),
        diff: e.diff,
        modDurationAtHorizon: e.modDurationAtHorizon,
      },
      floater:
        f === undefined
          ? null
          : {
              days: f.days,
              scenarios: f.scenarios.map((s) => {
                const r = { shiftPct: s.shiftPct, breakdown: breakdown(s.breakdown), coupons: s.coupons };
                s.free();
                return r;
              }),
            },
      offer: o === undefined ? null : { before: breakdown(o.before), after: breakdown(o.after) },
    };
    e.free();
    f?.free();
    o?.free();
    c.free();
    return out;
  };
  const result = (r, convert) => {
    const ok = r.ok;
    const out = ok === undefined ? { error: r.error } : { ok: convert(ok) };
    r.free();
    return out;
  };
  return {
    derive_bond: (issue, market) => {
      const i = issueOf(issue);
      const m = marketOf(market);
      const out = result(w.derive_bond(i, m), derived);
      i.free();
      m.free();
      return out;
    },
    calculate: (issue, market, plan) => {
      const i = issueOf(issue);
      const m = marketOf(market);
      const p = planOf(plan);
      const out = result(w.calculate(i, m, p), calculation);
      i.free();
      m.free();
      p.free();
      return out;
    },
  };
}
