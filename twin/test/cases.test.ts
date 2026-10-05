import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { compare, decodeNaN } from "../../node/compare.mjs";
import * as twin from "../src/index.js";
import type { Issue, Market, Plan } from "../src/index.js";

type Case = { name: string; fn: string; args: unknown[]; expect: unknown };

const cases = JSON.parse(readFileSync(new URL("../../cases.json", import.meta.url), "utf8")) as Case[];

const n = (v: unknown) => v as number;
const a = (v: unknown) => v as number[];

function run(fn: string, args: unknown[]): unknown {
  const x = decodeNaN(args) as unknown[];
  switch (fn) {
    case "price_from_yield":
      return twin.price_from_yield(a(x[0]), a(x[1]), n(x[2]));
    case "ytm_effective":
      return twin.ytm_effective(a(x[0]), a(x[1]), n(x[2]));
    case "ytm_simple":
      return twin.ytm_simple(a(x[0]), a(x[1]), n(x[2]));
    case "accrued_interest":
      return twin.accrued_interest(n(x[0]), n(x[1]), n(x[2]));
    case "macaulay_duration":
      return twin.macaulay_duration(a(x[0]), a(x[1]), n(x[2]));
    case "modified_duration":
      return twin.modified_duration(a(x[0]), a(x[1]), n(x[2]));
    case "build_cash_flow":
      return twin.build_cash_flow(
        n(x[0]),
        n(x[1]),
        a(x[2]),
        a(x[3]),
        a(x[4]),
        a(x[5]),
        n(x[6]),
        n(x[7]),
        n(x[8]),
      );
    case "floater_rate_path":
      return twin.floater_rate_path(n(x[0]), n(x[1]), n(x[2]), n(x[3]));
    case "floater_coupons":
      return twin.floater_coupons(n(x[0]), n(x[1]), a(x[2]), n(x[3]));
    case "tax_amount":
      return twin.tax_amount(n(x[0]), n(x[1]), n(x[2]), n(x[3]), n(x[4]));
    case "hold_value":
      return twin.hold_value(a(x[0]), a(x[1]), a(x[2]), n(x[3]), n(x[4]), n(x[5]));
    case "price_after_rate_shift":
      return twin.price_after_rate_shift(n(x[0]), n(x[1]), n(x[2]));
    case "periodic_rate_pct":
      return twin.periodic_rate_pct(n(x[0]), n(x[1]));
    case "value_along_path":
      return twin.value_along_path(a(x[0]), a(x[1]), n(x[2]), n(x[3]), a(x[4]));
    case "derive_bond":
      return twin.derive_bond(x[0] as Issue, x[1] as Market);
    case "calculate":
      return twin.calculate(x[0] as Issue, x[1] as Market, x[2] as Plan);
    default:
      throw new Error(`unknown function ${fn}`);
  }
}

describe("TypeScript twin against cases.json", () => {
  it("covers every function", () => {
    expect(new Set(cases.map((c) => c.fn)).size).toBe(16);
  });

  it.each(cases.map((c) => [c.name, c] as const))("%s", (_name, c) => {
    const report = compare(run(c.fn, c.args), c.expect, c.name);
    expect(report.failures).toEqual([]);
  });

  it("adds the breakdown lines up to the total", () => {
    for (const c of cases.filter((x) => x.fn === "calculate")) {
      const r = run(c.fn, c.args) as ReturnType<typeof twin.calculate>;
      if (!("ok" in r)) continue;
      const b = r.ok.plan;
      const sum = b.coupons + b.reinvest + b.amort + b.body + b.tax + b.commission;
      expect(Math.abs(sum - b.total)).toBeLessThanOrEqual(1e-9 * Math.max(Math.abs(b.total), 1));
      expect(b.tax).toBeLessThanOrEqual(0);
      expect(b.commission).toBeLessThanOrEqual(0);
    }
  });
});

describe("dates", () => {
  it("counts days across leap years", () => {
    expect(twin.parseIsoDate("1970-01-01")).toBe(0);
    expect(twin.parseIsoDate("2000-03-01")).toBe(11_017);
    expect(twin.parseIsoDate("0000-01-01")).toBe(-719_528);
    expect(twin.dayOffset("2027-09-04", "2028-09-04")).toBe(366);
  });

  it("rejects dates that do not exist or are malformed", () => {
    for (const s of ["", "2026-9-04", "2026-02-29", "2026-13-01", "2026-04-31", "2026-09-04T00:00:00Z"]) {
      expect(twin.parseIsoDate(s), s).toBeNull();
    }
  });
});
