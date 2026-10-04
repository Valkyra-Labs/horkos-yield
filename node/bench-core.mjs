// Measurement only: time of derive_bond and calculate inside wasm, with the
// 60-issue set already in wasm memory (no boundary crossing per issue).
import { readFileSync } from "node:fs";
import * as twin from "../twin/dist/index.js";
import { loadWasm } from "./wasm.mjs";

const RUNS = Number(process.argv[2] ?? 200);
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
const w = await loadWasm();
if (w.bench_hold(JSON.stringify(issues), JSON.stringify(plans)) !== 60) throw new Error("hold failed");
const best = [Infinity, Infinity];
for (let r = 0; r < 20 + RUNS; r++) {
  for (const mode of [0, 1]) {
    const t0 = performance.now();
    w.bench_run(mode, MARKET.valuationDate, MARKET.keyRatePct);
    const t = performance.now() - t0;
    if (r >= 20) best[mode] = Math.min(best[mode], t);
  }
}
console.log(`wasm core only: derive ${best[0].toFixed(3)} ms  calculate ${best[1].toFixed(3)} ms  both ${(best[0] + best[1]).toFixed(3)} ms`);
