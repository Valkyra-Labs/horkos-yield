# Measurements

Every number in the README and in the source comments comes from here,
with its stamp. Host: Apple M4 Pro (12 CPU cores), 24 GB, macOS 26.6
(26.6.2). Toolchain: Rust 1.98.0, wasm-pack 0.13.1 with the wasm-opt it
installs, wasm-bindgen 0.2.129, Node v22.18.0, TypeScript 6.0.3 for the
twin (target ES2022).

Builds:

- WebAssembly: `wasm-pack build --release --target web --out-dir pkg
  --out-name horkos_yield -- --no-default-features --features wasm`,
  release profile from the build named in each section.
- Twin: `pnpm build` in `twin/` (tsc to `twin/dist`).

Workload for every timing: `fixtures/issues60.json`, 60 fictional issues
(15 floaters, 13 amortising, 12 with an offer; coupon periods of 30, 91
and 182 days; maturities from 2026-12 to 2036-07), valuation date
2026-09-04, key rate 16 percent. One plan per issue: 100,000 invested,
horizon 365 days or the maturity day if sooner, coupons reinvested,
standard tax at 13 percent, key-rate shift +2 points. A run derives all 60
issues, then calculates the 60 plans (`calculate` derives the issue again
inside). Times are wall time from `performance.now()` and include building
the inputs from plain objects and turning the results into plain objects;
for the WebAssembly build that means setting struct fields, reading every
getter and freeing every struct (`node/wasm.mjs`). Before timing,
`node/bench.mjs` checks that the two implementations agree within the
parity tolerance on every issue.

Each table row is one invocation; `best` is the best of N runs after 20
warm-up runs, with N stated. "derive" and "calculate" are the best of each
pass on its own; "both" is the best run of the two passes together, not
the sum of the two bests.

## Size and speed (build `36282c7`)

Release profile: opt-level 3, LTO, one codegen unit.

| file | bytes | gzip level 9 (Node zlib) |
|---|---|---|
| `pkg/horkos_yield_bg.wasm` | 64,861 | 24,975 |
| `pkg/horkos_yield.js` (wasm-bindgen glue) | 57,306 | 6,661 |
| `twin/dist/*.js` (6 files) | 18,827 | 5,414 (concatenated) |

`node node/bench.mjs 500`, best of 500, milliseconds for the whole set:

| run | twin derive | twin calculate | twin both | wasm derive | wasm calculate | wasm both |
|---|---|---|---|---|---|---|
| 1 | 3.806 | 3.995 | 7.908 | 4.709 | 4.840 | 9.603 |
| 2 | 3.976 | 4.133 | 8.127 | 4.698 | 4.779 | 9.567 |
| 3 | 3.947 | 4.088 | 8.100 | 4.585 | 4.748 | 9.453 |

Under Node the twin is faster than the WebAssembly build on this
workload: 14 to 18 percent less time on "both". Where the time goes inside
either implementation was not profiled.

What this does not show: browsers (only Node's V8 was measured), module
compile and instantiation time, the first call before warm-up, memory,
and schedules much longer than the fixture's (at most 120 coupons).

## WebAssembly boundary (build `4dfc2a9`)

At `4dfc2a9` the crate exported `derive_bond` and `calculate` twice: as
JSON strings (a serde-free reader and writer; NaN and infinities crossed
as strings and a `JSON.parse` reviver turned them back into numbers) and
as wasm-bindgen structs. `node/bench-core.mjs` (removed afterwards) timed
the same work with the 60 issues and plans held in wasm memory, so no
boundary was crossed per issue; its "both" is the sum of the best derive
and the best calculate. Release profile: the default (opt-level 3, no
LTO). Best of 500, milliseconds for the whole set:

| run | inside wasm | struct boundary | JSON boundary | struct overhead | JSON overhead |
|---|---|---|---|---|---|
| 1 | 9.255 | 9.611 | 12.554 | +0.356 (3.8%) | +3.299 (35.6%) |
| 2 | 9.011 | 9.532 | 12.429 | +0.521 (5.8%) | +3.418 (37.9%) |
| 3 | 8.838 | 9.392 | 12.417 | +0.554 (6.3%) | +3.579 (40.5%) |

Per pass, at run 1: derive 4.556 inside, 4.678 structs, 6.384 JSON;
calculate 4.699 inside, 4.855 structs, 6.088 JSON.

Choice: structs. The JSON boundary also needed the special-number
convention above. With both boundaries and the measurement exports the
module was 137,466 bytes (60,513 gzipped); with only the structs, at
`fd23e70` and the same profile, 68,932 (26,446). That difference covers
the JSON reader and writer, Rust's float formatting and the measurement
exports together; they were not measured separately.

## Release profile (build `fd23e70`)

The same build, with the profile set through `CARGO_PROFILE_RELEASE_*`
environment variables, one `node node/bench.mjs 300` each:

| opt-level | LTO | codegen units | bytes | gzip level 9 | wasm both (ms) |
|---|---|---|---|---|---|
| 3 | off | 16 | 68,932 | 26,446 | 9.627 |
| 3 | on | 1 | 64,861 | 24,975 | 9.607 |
| s | on | 1 | 64,454 | 25,574 | 9.751 |
| z | on | 1 | 64,571 | 25,636 | 9.962 |

A repeat of the first row gave 9.600 ms. Chosen: opt-level 3 with LTO and
one codegen unit, the smallest gzipped module and as fast as the default.

## Parity (build `36282c7`)

Tolerance: `|a - b| <= 1e-6 * max(|a|, |b|)`; both NaN equal; strings,
booleans and nulls exact. No case needed a wider tolerance.

| check | cases | failures | largest relative difference among agreeing numbers |
|---|---|---|---|
| Rust (native, `cargo test --release`) against `cases.json` | 97 | 0 | 4.3e-15 (a reinvestment income in a floater case) |
| twin (vitest) against `cases.json` | 97 | 0 | not reported by the test |
| wasm against the table, twin against the table, wasm against twin (`node --test node/parity.test.mjs`) | 97 | 0 | 0: identical |
| wasm against twin, generated issues and plans (seed 20261004) | 1,000 | 0 | 1.8e-11 (an early-exit `diff`, a difference of two totals) |

Of the 1,000 generated issues, 942 derive and 846 plans calculate; the
rest are error outcomes, on which the two implementations return the
same code. Error outcomes counted over both calls: `matured` 52,
`amount_below_one_bond` 48, `horizon_out_of_range` 30, `invalid_code` 21,
`invalid_period` 21, `invalid_nominal` 20, `invalid_date` 8,
`amount_not_positive` 6, `amount_too_large` 3, `invalid_price` 3.

What this does not show: agreement outside the generator's ranges
(valuation dates 2020 to 2034, maturities up to ten years, prices 40 to
130 percent of nominal), or agreement of the primitives on random inputs
(they are compared on the table cases only).
