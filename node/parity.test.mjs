// Rust (the wasm build in pkg/) against the TypeScript twin (twin/dist):
// every case in cases.json, then 1,000 generated issues with a plan each.
// Run after building both:  node --test node/
//
// Tolerance: numbers agree when |a - b| <= 1e-6 * max(|a|, |b|), both NaN
// counts as equal, everything else must be identical (node/compare.mjs).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import * as twin from "../twin/dist/index.js";
import { compare, decodeNaN } from "./compare.mjs";
import { loadWasm, wrap } from "./wasm.mjs";

const cases = JSON.parse(readFileSync(new URL("../cases.json", import.meta.url), "utf8"));
const glue = await loadWasm();
const wasm = { ...glue, ...wrap(glue) };

const ARRAY_ARGS = {
  price_from_yield: [0, 1],
  ytm_effective: [0, 1],
  ytm_simple: [0, 1],
  macaulay_duration: [0, 1],
  modified_duration: [0, 1],
  build_cash_flow: [2, 3, 4, 5],
  floater_coupons: [2],
  hold_value: [0, 1, 2],
};

function call(impl, fn, args) {
  const typed = ARRAY_ARGS[fn] ?? [];
  const x = decodeNaN(args).map((a, i) => (typed.includes(i) ? Float64Array.from(a) : a));
  return impl[fn](...x);
}

test("wasm and twin agree with cases.json and with each other on every case", () => {
  const failures = [];
  let worst = 0;
  for (const c of cases) {
    const w = call(wasm, c.fn, c.args);
    const t = call(twin, c.fn, c.args);
    for (const [label, got, want] of [
      ["wasm vs table", w, c.expect],
      ["twin vs table", t, c.expect],
      ["wasm vs twin", w, t],
    ]) {
      const r = compare(got, want, `${label}: ${c.name}`);
      failures.push(...r.failures);
      worst = Math.max(worst, r.worst);
    }
  }
  console.log(`${cases.length} cases; worst relative difference ${worst.toExponential(2)}`);
  assert.deepEqual(failures, []);
});

// mulberry32: a small seeded generator, so the set is the same on every run.
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// YYYY-MM-DD of a day number (days since 1970-01-01).
function isoOf(days) {
  return new Date(days * 86_400_000).toISOString().slice(0, 10);
}

export function generate(count, seed) {
  const r = rng(seed);
  const uniform = (lo, hi) => lo + (hi - lo) * r();
  const int = (lo, hi) => Math.floor(uniform(lo, hi + 1));
  const pick = (xs) => xs[Math.floor(r() * xs.length)];
  const chance = (p) => r() < p;
  const round = (x, d) => Math.round(x * 10 ** d) / 10 ** d;
  const out = [];
  for (let k = 0; k < count; k++) {
    const today = int(18_262, 23_741); // 2020-01-01 to 2034-12-31
    const period = chance(0.05) ? int(1, 400) : pick([30, 91, 182, 365]);
    const maturityDay = chance(0.02) ? int(-400, 0) : Math.round(Math.exp(uniform(0, Math.log(3650))));
    const couponDays = [];
    for (let d = maturityDay; d > 0; d -= period) couponDays.unshift(d);
    const floater = chance(0.3);
    const offers = [];
    for (let i = int(0, 3); i > 0; i--) {
      offers.push(isoOf(today + (chance(0.6) && couponDays.length ? pick(couponDays) : int(-200, maturityDay + 100))));
    }
    const amortization = [];
    if (chance(0.3) && couponDays.length >= 2) {
      const tranches = int(1, Math.min(6, couponDays.length));
      for (let i = couponDays.length - tranches; i < couponDays.length; i++) {
        amortization.push({ date: isoOf(today + couponDays[i]), fractionPct: round(100 / tranches, 2) });
      }
      if (chance(0.2)) amortization.push({ date: isoOf(today + int(1, maturityDay + 30)), fractionPct: 10 });
    }
    const issue = {
      nominal: chance(0.01) ? pick([0, -1000, Number.NaN]) : pick([1000, 1000, 1000, 500, 10_000]),
      pricePct: chance(0.01) ? pick([0, -20]) : round(uniform(40, 130), 2),
      accrued: chance(0.25) ? null : round(uniform(0, 60), 2),
      couponType: chance(0.005) ? "zero" : floater ? "floater" : "fixed",
      couponRatePct: round(uniform(0, 25), 2),
      spreadPct: floater ? round(uniform(-1, 8), 2) : 0,
      periodDays: chance(0.005) ? 0 : period,
      maturity: chance(0.005) ? `${isoOf(today + maturityDay).slice(0, 8)}32` : isoOf(today + maturityDay),
      offers,
      amortization,
    };
    const market = { valuationDate: isoOf(today), keyRatePct: round(uniform(4, 25), 2) };
    const plan = {
      amount: chance(0.02) ? pick([0, -5, Number.NaN, 2e9, 50]) : Math.round(Math.exp(uniform(Math.log(1000), Math.log(5e6)))),
      horizonDay: chance(0.03) ? pick([0, 0.5, maturityDay + 1, Number.NaN]) : int(1, Math.max(1, maturityDay)),
      reinvest: chance(0.5),
      taxRegime: chance(0.005) ? "flat" : pick(["standard", "ldv", "iis_b"]),
      taxRatePct: pick([13, 15]),
      rateShiftPct: chance(0.3) ? 0 : round(uniform(-3, 3), 2),
    };
    out.push({ issue, market, plan });
  }
  return out;
}

test("wasm and twin agree on 1,000 generated issues and plans", () => {
  const set = generate(1000, 20261004);
  const failures = [];
  let worst = 0;
  let worstAt = "";
  const outcomes = { derived: 0, calculated: 0, errors: {} };
  for (const [k, { issue, market, plan }] of set.entries()) {
    const pairs = [
      ["derive_bond", wasm.derive_bond(issue, market), twin.derive_bond(issue, market)],
      ["calculate", wasm.calculate(issue, market, plan), twin.calculate(issue, market, plan)],
    ];
    for (const [fn, w, t] of pairs) {
      const r = compare(w, t, `issue ${k} ${fn}`);
      failures.push(...r.failures);
      if (r.worst > worst) [worst, worstAt] = [r.worst, r.worstAt];
      if ("ok" in t) outcomes[fn === "derive_bond" ? "derived" : "calculated"] += 1;
      else outcomes.errors[t.error] = (outcomes.errors[t.error] ?? 0) + 1;
    }
  }
  console.log(
    `1,000 issues: ${outcomes.derived} derived, ${outcomes.calculated} calculated; errors ${JSON.stringify(outcomes.errors)}; worst relative difference ${worst.toExponential(2)} at ${worstAt}`,
  );
  assert.deepEqual(failures.slice(0, 20), []);
  // The set must exercise the paths, not only the errors.
  assert.ok(outcomes.derived > 900 && outcomes.calculated > 800);
});
