/*
  horkos-yield-twin: the TypeScript implementation of horkos-yield. Same
  functions, same names, same results within the stated tolerance; written
  separately so each implementation checks the other.
*/

export * from "./primitives.js";
export { coupon_schedule, derive_bond } from "./issue.js";
export {
  COMMISSION_PCT,
  FLOATER_RAMP_STEPS,
  FLOATER_SHIFTS_PCT,
  MAX_AMOUNT,
  MIN_ANNUALISED_DAYS,
  WORST_CASE_COUPON_PCT,
  calculate,
  effective_annual_pct,
} from "./calculate.js";
export { civilFromDays, dayOffset, parseIsoDate } from "./dates.js";
export type * from "./types.js";
