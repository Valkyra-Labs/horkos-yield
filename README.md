# horkos-yield

[![CI](https://github.com/Valkyra-Labs/horkos-yield/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Valkyra-Labs/horkos-yield/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Valkyra-Labs/horkos-yield/badges/tests.json)](https://github.com/Valkyra-Labs/horkos-yield/actions/workflows/ci.yml)
[![Twin tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Valkyra-Labs/horkos-yield/badges/twin-tests.json)](https://github.com/Valkyra-Labs/horkos-yield/actions/workflows/ci.yml)
[![Parity](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Valkyra-Labs/horkos-yield/badges/parity.json)](#parity)
[![wasm gzip](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Valkyra-Labs/horkos-yield/badges/wasm-size.json)](https://github.com/Valkyra-Labs/horkos-yield/actions/workflows/ci.yml)
[![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue.svg)](Cargo.toml)

The tests, twin tests, parity and wasm badges are published by CI from
each green run on `main`: tests passed in `cargo test --release` on
Linux (unit, integration and doc tests); tests passed in the twin's
`pnpm test` (Vitest); the number of cases and of generated issues on
which `node/parity.test.mjs` found the WebAssembly build and the twin in
agreement (see [Parity](#parity)), as each test reports it once it has
passed; and the gzip size (level 9) of the WebAssembly module that CI
builds with wasm-pack (`--no-default-features --features wasm`). CI
checks the MSRV with `cargo +1.85 check`.

Bond mathematics for a bond-investing demo, in Rust compiled to
WebAssembly, with a TypeScript twin in `twin/`: a second implementation
of the same functions, written separately and checked against the Rust
one on every case.

Status: early. Both implementations pass the 109 cases in `cases.json`
and the hand-computed worked examples,
and the WebAssembly build agrees with the twin on those cases and on
1,000 generated issues within 1e-6 relative. Sizes, timings and how the
WebAssembly boundary was chosen: [docs/MEASUREMENTS.md](docs/MEASUREMENTS.md).

## What it computes

Primitives, on flat arrays of numbers:

- price from an annual effective yield; yield to maturity, effective
  (bisection on -99 to 1000 percent, 200 steps), and the simple yield
  over the full term (all payments less the price, over the price,
  divided by the years to the last payment; not compounded, and well
  below the yield for an amortising issue, whose principal comes back
  early);
- accrued interest; Macaulay and modified duration;
- the cash-flow schedule with amortisation and an offer (redeemed at the
  offer, or a new coupon rate after it);
- floater key-rate paths and coupons; a yield as the rate compounded
  once a coupon period, and the value of flows discounted along a path
  of per-period rates;
- personal income tax on a year's base: 13 percent up to 2.4 million
  roubles of the year's investment income, 15 percent above, the
  threshold shared with the holder's other investment income;
- what a holder collects by a horizon (coupons, income from reinvesting
  the coupons and the principal repaid early, amortisation, final
  redemption, sale value), and the price after a parallel rate shift.

For an issue:

- `derive_bond(issue, market)`: the coupon schedule counted back from
  maturity, the flows to maturity and to the nearest offer, accrued
  interest, the dirty price, yields to maturity and to the offer, the
  simple yield, and durations.
- `calculate(issue, market, plan)`: for an amount, a horizon,
  reinvestment on or off, the account (ordinary or IIS type B), the
  holder's other investment income and a key-rate change by the horizon:
  the plan's totals, its return over the period and, for a horizon of 30
  days or more, its effective annual return, as a signed breakdown; the
  early exit under the key-rate change; three floater scenarios (key rate
  -2, 0 and +2 points, reached over four coupon periods); and, for an
  issue with an offer, holding to the offer against holding through it
  at a 0.1 percent coupon.

How `calculate` models the holding:

- Coupons and principal repaid before the horizon are reinvested, when
  the plan asks, at the yield to maturity.
- A key-rate change moves a fixed coupon's sale price by its modified
  duration. A floater's coupons follow the key rate, and its sale keeps
  today's spread to the key rate (the flows are discounted along the
  key-rate path at that spread), so its price stays close to where it
  is; the early exit gives no duration for a floater.
- Tax, in an ordinary account, is counted per calendar year: coupons and
  the result of redemptions and the sale form one base, so a loss
  reduces that year's tax on coupons; a negative year pays nothing and
  is not carried to another year. The accrued interest paid at purchase
  reduces the first coupon received and the cost by the same amount, or
  stays in the cost when no coupon is received; commissions are costs.
  A redemption or sale more than three years after the purchase,
  counted by calendar anniversary, has its positive result exempt (the
  long-term holding relief), up to 3 million roubles for each full year
  held; coupons stay taxed. Rates as above. Reinvestment income is taxed
  in the horizon's year.
- IIS type B (only accounts opened by the end of 2023) is taken as no
  tax: income in it is free of tax when it is closed after at least
  three years.
- The return is not annualised for a horizon under 30 days, where
  compounding to a year turns small amounts such as the commission into
  large annual rates.

The tax rules follow the Tax Code of the Russian Federation, part two,
articles 214.1, 219.1 and 224, as in force from 1 October 2026.

Conventions: days are whole-day offsets from the valuation date, ACT/365;
amounts are per bond in currency units unless the field is a total; rates
ending in `_pct` (`Pct` in JavaScript) are percents, others are
fractions. In a breakdown, income lines are positive and costs (`tax`,
`commission`) negative, and `total` is their sum. Commission is 0.05
percent on the purchase and on a sale before redemption. Outputs are
numbers, codes and day offsets; there is no human-language text.

Errors are values. `derive_bond` and `calculate` return an error code, in
this order of checks:

| code | when |
|---|---|
| `invalid_code` | coupon type or tax regime is not a known code (JavaScript only) |
| `invalid_date` | a date is not a valid `YYYY-MM-DD` |
| `invalid_nominal` | nominal is not a positive finite number |
| `invalid_period` | coupon period is not a finite number of at least one day |
| `matured` | maturity is on or before the valuation date |
| `amount_not_positive` | amount is not a positive finite number |
| `amount_too_large` | amount is above 1e9 |
| `horizon_out_of_range` | horizon is not between day 1 and maturity |
| `invalid_other_income` | other investment income is not a finite number of at least zero |
| `invalid_price` | dirty price is not a positive finite number |
| `amount_below_one_bond` | the amount does not buy one bond |

The primitives return NaN for invalid inputs (no flows, a price that is
not positive, a NaN argument), and `derive_bond` keeps that: a price that
is not positive gives NaN yields, not an error. Nothing panics on any
input.

Limits: coupons fall at a fixed period in days, not on calendar months;
amortisation pays only on a coupon day, and amortisation before the
valuation date does not reduce the nominal; the purchase settles on the
valuation date and a sale on the horizon; tax is counted at the horizon
rather than withheld coupon by coupon; carrying a loss to a later year
(by declaration) is not modelled; reinvestment and the sale both use the
yield to maturity, which for a floater assumes the key rate stays where
it is.

## API

Rust:

```rust
use horkos_yield::{calculate, derive_bond, CouponType, Issue, Market, Plan, TaxRegime};

let issue = Issue {
    nominal: 1000.0,
    price_pct: 98.12,
    accrued: None, // None: computed from the schedule
    coupon_type: CouponType::Fixed,
    coupon_rate_pct: 14.0,
    spread_pct: 0.0,
    period_days: 182.0,
    maturity: "2029-01-12".into(),
    offers: vec![],
    amortization: vec![],
};
let market = Market { valuation_date: "2026-09-04".into(), key_rate_pct: 16.0 };
let derived = derive_bond(&issue, &market)?;

let plan = Plan {
    amount: 100_000.0,
    horizon_day: 365.0,
    reinvest: true,
    tax_regime: TaxRegime::Standard,
    other_income: 0.0,
    rate_shift_pct: 2.0,
};
let result = calculate(&issue, &market, &plan)?;
```

The primitives (`price_from_yield`, `ytm_effective`, `ytm_simple`,
`accrued_interest`, `macaulay_duration`, `modified_duration`,
`build_cash_flow`, `floater_rate_path`, `floater_coupons`,
`periodic_rate_pct`, `value_along_path`, `income_tax`,
`hold_value`, `price_after_rate_shift`) and `effective_annual_pct` are
re-exported at the crate root. Types are plain structs without serde.

JavaScript, from the WebAssembly package: the primitives under the same
names on `Float64Array`s; `derive_bond` and `calculate` on wasm-bindgen
structs with camelCase fields. Results have `ok` or `error` set; arrays
come back as `Float64Array`; each struct read from a result is a copy to
`free()` when done.

```js
import init, { Issue, Market, derive_bond } from "./pkg/horkos_yield.js";

await init();
const issue = new Issue();
issue.nominal = 1000;
issue.pricePct = 98.12;
issue.couponType = "fixed";
issue.couponRatePct = 14;
issue.periodDays = 182;
issue.maturity = "2029-01-12";
const market = new Market();
market.valuationDate = "2026-09-04";
market.keyRatePct = 16;
const r = derive_bond(issue, market);
console.log(r.error ?? r.ok.ytmMaturity);
```

`node/wasm.mjs` wraps the package behind the twin's API (plain objects
in and out, every struct freed); the parity test and the benchmark use
it.

TypeScript twin (`twin/`, package `horkos-yield-twin`, no runtime
dependencies): the same functions under the same names, taking and
returning plain objects; results are `{ ok }` or `{ error }`.

```ts
import { derive_bond } from "horkos-yield-twin";

const r = derive_bond(
  {
    nominal: 1000, pricePct: 98.12, accrued: null, couponType: "fixed",
    couponRatePct: 14, spreadPct: 0, periodDays: 182, maturity: "2029-01-12",
    offers: [], amortization: [],
  },
  { valuationDate: "2026-09-04", keyRatePct: 16 },
);
if ("ok" in r) console.log(r.ok.ytmMaturity);
```

## Build

```bash
cargo test --release
wasm-pack build --release --target web --out-dir pkg --out-name horkos_yield -- --no-default-features --features wasm
```

The twin (pnpm, Node 22):

```bash
cd twin
pnpm install --frozen-lockfile
pnpm build       # tsc to twin/dist
pnpm typecheck
pnpm test        # vitest against cases.json
```

## Parity

- `cases.json` holds 109 cases: 42 for the primitives (six of them
  edge cases), 25 for `derive_bond` and 42 for `calculate`
  (amortisation, offers, floaters, both accounts, the 15 percent rate,
  the long-term holding relief on either side of the third anniversary
  and at its cap, moved valuation dates and every error code). NaN is
  written `"NaN"`. The expected values for `derive_bond` were computed by
  the TypeScript code these functions were ported from, not by either
  implementation here. Those for `calculate` were computed the same way
  until the tax, floater and reinvestment model changed; the cases it
  changed were recomputed by the twin and the Rust crate agrees with
  them. The new primitive cases and the error expectations are written by
  hand.
- `tests/worked.rs` and `twin/test/worked.test.ts` run the same worked
  examples, each a small issue whose results are computed by hand with
  the arithmetic in comments: a floater under a key-rate change, an
  amortising plan, tax netting with the accrued interest paid, the
  15 percent rate, the long-term holding relief and its cap, and the
  shortest annualised horizon.
- `tests/cases.rs` checks the Rust crate against the table;
  `twin/test/cases.test.ts` checks the twin.
- `node/parity.test.mjs` loads the built package and the built twin and
  checks, on every case, the WebAssembly build against the table, the
  twin against the table and the two against each other; then the two
  against each other on 1,000 issues and plans from a seeded generator,
  including invalid inputs. Build `pkg/` and `twin/dist` first, then:

  ```bash
  node --test node/parity.test.mjs
  ```

Tolerance everywhere: `|a - b| <= 1e-6 * max(|a|, |b|)`, so an expected
zero must be exactly zero; both NaN counts as equal; strings, booleans
and nulls compare exactly. No case needs a wider tolerance.

`node node/bench.mjs [runs]` times both implementations on the 60
fictional issues in `fixtures/issues60.json`.

## License

MIT OR Apache-2.0, at your option.
