// Times derive_bond and calculate over the 60-issue fixture, in the wasm
// build (pkg/) and in the twin (twin/dist), under Node.
//
//   node node/bench.mjs [runs]
//
// Each run derives all 60 issues, then calculates one plan for each; the
// report gives the best run of N (after 20 warm-up runs) for the derive
// pass, the calculate pass and the two together. Times include building
// the inputs and turning the results into plain objects.

import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import * as twin from "../twin/dist/index.js";
import { compare } from "./compare.mjs";
import { jsonBoundary, loadWasm, structBoundary } from "./wasm.mjs";

const RUNS = Number(process.argv[2] ?? 200);
const WARMUP = 20;
const MARKET = { valuationDate: "2026-09-04", keyRatePct: 16 };
const issues = JSON.parse(readFileSync(new URL("../fixtures/issues60.json", import.meta.url), "utf8"));
const plans = issues.map((i) => ({
  amount: 100_000,
  horizonDay: Math.min(365, twin.dayOffset(MARKET.valuationDate, i.maturity)),
  reinvest: true,
  taxRegime: "standard",
  taxRatePct: 13,
  rateShiftPct: 2,
}));

function pass(impl) {
  const t0 = performance.now();
  const derived = issues.map((i) => impl.derive_bond(i, MARKET));
  const t1 = performance.now();
  const calculated = issues.map((i, k) => impl.calculate(i, MARKET, plans[k]));
  const t2 = performance.now();
  return { derive: t1 - t0, calculate: t2 - t1, both: t2 - t0, derived, calculated };
}

function bench(impl) {
  for (let i = 0; i < WARMUP; i++) pass(impl);
  const best = { derive: Infinity, calculate: Infinity, both: Infinity };
  for (let i = 0; i < RUNS; i++) {
    const r = pass(impl);
    for (const k of Object.keys(best)) best[k] = Math.min(best[k], r[k]);
  }
  return best;
}

const wasm = await loadWasm();
const bytes = readFileSync(new URL("../pkg/horkos_yield_bg.wasm", import.meta.url));
const impls = {
  twin: { derive_bond: twin.derive_bond, calculate: twin.calculate },
  "wasm, JSON boundary": jsonBoundary(wasm),
  "wasm, struct boundary": structBoundary(wasm),
};

// The implementations must agree before their times mean anything.
const reference = pass(impls.twin);
for (const [name, impl] of Object.entries(impls)) {
  const r = pass(impl);
  const report = compare([r.derived, r.calculated], [reference.derived, reference.calculated], name);
  if (report.failures.length) throw new Error(`${name} disagrees with the twin:\n${report.failures.slice(0, 5).join("\n")}`);
}

const errors = reference.calculated.filter((r) => "error" in r).length;
const commit = execSync("git describe --always --dirty", { encoding: "utf8" }).trim();
console.log(`commit ${commit}, node ${process.version}, ${RUNS} runs after ${WARMUP} warm-up`);
console.log(`wasm ${bytes.length} bytes, gzip -9 ${gzipSync(bytes, { level: 9 }).length} bytes`);
console.log(`60 issues, ${60 - errors} plans calculated, ${errors} plan errors`);
for (const [name, impl] of Object.entries(impls)) {
  const b = bench(impl);
  console.log(
    `${name.padEnd(22)} derive ${b.derive.toFixed(3)} ms  calculate ${b.calculate.toFixed(3)} ms  both ${b.both.toFixed(3)} ms`,
  );
}
