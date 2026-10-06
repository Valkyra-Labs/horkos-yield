/*
  Data shapes shared with the Rust crate. Field names are the camelCase
  forms of the Rust fields; days are offsets from the valuation date.
*/

export type Num = ArrayLike<number>;

export type ErrorCode =
  | "invalid_code"
  | "invalid_date"
  | "invalid_nominal"
  | "invalid_period"
  | "matured"
  | "amount_not_positive"
  | "amount_too_large"
  | "horizon_out_of_range"
  | "invalid_other_income"
  | "invalid_price"
  | "amount_below_one_bond";

/* Errors are values: either `ok` or `error` is present */
export type Result<T> = { ok: T } | { error: ErrorCode };

export type CouponType = "fixed" | "floater";
/*
  standard: an ordinary brokerage account, with the long-term holding relief
  applied by itself; iis_b: an individual investment account of type B
  (accounts opened by the end of 2023), taken as no tax
*/
export type TaxRegime = "standard" | "iis_b";
export type EventKind = "offer" | "maturity";

export type Amortization = {
  /* YYYY-MM-DD; pays only when it falls on a coupon day */
  date: string;
  /* Share of the original nominal, percent */
  fractionPct: number;
};

export type Issue = {
  nominal: number;
  /* Clean price, percent of nominal */
  pricePct: number;
  /* Quoted accrued interest per bond; null uses the computed value */
  accrued: number | null;
  couponType: CouponType;
  /* Annual coupon rate, percent; fixed coupons only */
  couponRatePct: number;
  /* Spread over the key rate, percent; floaters only */
  spreadPct: number;
  periodDays: number;
  /* YYYY-MM-DD */
  maturity: string;
  /* YYYY-MM-DD, any order */
  offers: string[];
  amortization: Amortization[];
};

export type Market = {
  /* YYYY-MM-DD; every day offset counts from here */
  valuationDate: string;
  keyRatePct: number;
};

export type Plan = {
  amount: number;
  /* Day offset of the horizon, 1 to the maturity day */
  horizonDay: number;
  /* Reinvest coupons and principal repaid by the horizon at the yield to maturity */
  reinvest: boolean;
  taxRegime: TaxRegime;
  /*
    The holder's other investment income in each calendar year: with this
    position's income it decides how much is taxed at 15 rather than 13
    percent
  */
  otherIncome: number;
  /* Key-rate change by the horizon for the early exit, percentage points */
  rateShiftPct: number;
};

export type Schedule = { days: number[]; coupons: number[]; principals: number[] };

export type Derived = {
  maturityDay: number;
  couponDays: number[];
  ratesPct: number[];
  amortDays: number[];
  amortFracs: number[];
  daysSinceLast: number;
  couponAmount: number;
  accrued: number;
  dirtyPrice: number;
  flows: Schedule;
  flowsToOffer: Schedule | null;
  offerDay: number | null;
  ytmMaturity: number;
  ytmOffer: number | null;
  ytmSimple: number;
  event: EventKind;
  eventDay: number;
  yieldEvent: number;
  macaulay: number;
  modified: number;
};

/* Income lines positive, costs (tax, commission) negative; total is their sum */
export type Breakdown = {
  qty: number;
  invested: number;
  coupons: number;
  reinvest: number;
  amort: number;
  body: number;
  tax: number;
  commission: number;
  total: number;
  profit: number;
  /* Return over the holding period, percent: profit / invested */
  periodPct: number;
  /* Effective annual return, percent; null under MIN_ANNUALISED_DAYS */
  annualPct: number | null;
  horizonDay: number;
};

export type EarlyExit = {
  rateShiftPct: number;
  applicable: boolean;
  result: Breakdown;
  diff: number;
  /* Fixed coupons only; null for a floater, whose sale is not priced by duration */
  modDurationAtHorizon: number | null;
};

export type FloaterScenario = { shiftPct: number; breakdown: Breakdown; coupons: number[] };

export type Calculation = {
  plan: Breakdown;
  earlyExit: EarlyExit;
  floater: { days: number[]; scenarios: FloaterScenario[] } | null;
  offer: { before: Breakdown; after: Breakdown } | null;
};
