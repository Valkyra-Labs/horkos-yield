// Shields.io endpoint badges from what CI measured on this commit.
//
// Usage (CI runs it after the tests, the WebAssembly build and the parity
// run; see .github/workflows/ci.yml):
//   node scripts/badges.mjs --out <dir>
//     --tests <output of `cargo test --release`>
//     --twin <output of `pnpm test` in twin/>
//     --parity <TAP output of `node --test --test-reporter=tap node/parity.test.mjs`>
//     --wasm <pkg/horkos_yield_bg.wasm>
//
// Each badge is one JSON file, {"schemaVersion":1,"label","message","color"},
// which CI commits to the `badges` branch for img.shields.io/endpoint to
// read. A value that cannot be read, or a run that did not pass, stops the
// script with an error: a badge is never written from a guess. No
// dependencies: Node 18 or later.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";

class BadgeError extends Error {}
const fail = (message) => {
  throw new BadgeError(message);
};

const readBytes = (path) => {
  try {
    return readFileSync(path);
  } catch (e) {
    return fail(`cannot read ${path}: ${e.message}`);
  }
};
const readLines = (path) => readBytes(path).toString("utf8").replace(ANSI, "").split(/\r?\n/);

const ANSI = /\x1b\[[0-9;]*[A-Za-z]/g;
const BINARY = /^\s*(Running|Doc-tests) (.+)$/;
const RESULT = /^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/;

/** Test counts from `cargo test` output: one "test result" line for every
 * test binary and doc-test run that cargo announced. */
export function parseCargoTest(lines) {
  let binaries = 0;
  const results = [];
  for (const raw of lines) {
    const line = raw.trim();
    if (BINARY.test(line)) binaries++;
    const m = RESULT.exec(line);
    if (m) results.push({ ok: m[1] === "ok", passed: +m[2], failed: +m[3], ignored: +m[4], filtered: +m[6] });
  }
  if (binaries === 0) fail("no test binary in the cargo test output");
  if (results.length !== binaries) fail(`cargo announced ${binaries} test runs but printed ${results.length} results`);
  const sum = (key) => results.reduce((n, r) => n + r[key], 0);
  if (results.some((r) => !r.ok) || sum("failed") > 0) fail(`${sum("failed")} test(s) failed`);
  if (sum("filtered") > 0) fail("tests were filtered out: not a full run");
  if (sum("passed") === 0) fail("no test passed");
  return { passed: sum("passed"), ignored: sum("ignored") };
}

const SUMMARY = (name) => new RegExp(`^\\s*${name}\\s+(.+?)\\s+\\((\\d+)\\)\\s*$`);

/** The counts on one Vitest summary line ("Test Files" or "Tests"); there
 * must be exactly one, and its parts must add up to its total. */
function vitestLine(lines, name, kinds) {
  const re = SUMMARY(name);
  const found = lines.filter((l) => re.test(l));
  if (found.length !== 1) fail(`expected one Vitest "${name}" summary in the twin's test log, found ${found.length}`);
  const [, parts, total] = re.exec(found[0]);
  const counts = Object.fromEntries(kinds.map((k) => [k, 0]));
  for (const part of parts.split("|")) {
    const p = new RegExp(`^\\s*(\\d+) (${kinds.join("|")})\\s*$`).exec(part);
    if (!p) fail(`unrecognised Vitest summary: "${found[0].trim()}"`);
    counts[p[2]] += Number(p[1]);
  }
  if (Object.values(counts).reduce((a, b) => a + b, 0) !== Number(total)) fail(`Vitest summary does not add up: "${found[0].trim()}"`);
  return counts;
}

/** Counts from the one Vitest summary in the output of `pnpm test`. A test
 * file that failed to load counts as a failure even when no test in it
 * ran. */
export function parseVitest(lines) {
  const files = vitestLine(lines, "Test Files", ["passed", "failed", "skipped"]);
  if (files.failed > 0) fail(`${files.failed} twin test file(s) failed`);
  const counts = vitestLine(lines, "Tests", ["passed", "failed", "skipped", "todo"]);
  if (counts.failed > 0) fail(`${counts.failed} twin test(s) failed`);
  if (counts.passed === 0) fail("no twin test passed");
  return { passed: counts.passed, skipped: counts.skipped + counts.todo };
}

const TAP_POINT = /^(ok|not ok) \d+ - (.+)$/;
const TAP_COUNT = /^# (tests|pass|fail|cancelled|skipped|todo) (\d+)$/;
const PARITY = /^# parity (\{.*\})$/;
const PARITY_SETS = ["cases", "generated issues"];

/** What the parity run checked, from its TAP output: every test passed,
 * none skipped, and each passing test reported the size of its set, once.
 * The count is the set's own length, not a number written in a title. */
export function parseParity(lines) {
  const totals = {};
  const checked = {};
  let last = null;
  for (const line of lines) {
    const point = TAP_POINT.exec(line);
    if (point) last = { ok: point[1] === "ok", title: point[2] };
    const count = TAP_COUNT.exec(line);
    if (count) totals[count[1]] = Number(count[2]);
    const parity = PARITY.exec(line);
    if (parity) {
      if (!last?.ok) fail("a parity count follows a test that did not pass");
      let report;
      try {
        report = JSON.parse(parity[1]);
      } catch (e) {
        return fail(`unreadable parity count "${parity[1]}": ${e.message}`);
      }
      if (!PARITY_SETS.includes(report.checked) || !Number.isInteger(report.count)) fail(`unrecognised parity count "${parity[1]}"`);
      if (report.checked in checked) fail(`the parity run reported "${report.checked}" twice`);
      checked[report.checked] = report.count;
    }
  }
  for (const key of ["tests", "pass", "fail", "cancelled", "skipped", "todo"]) {
    if (!(key in totals)) fail(`the parity output has no "# ${key}" total`);
  }
  if (totals.fail > 0 || totals.cancelled > 0) fail(`${totals.fail + totals.cancelled} parity test(s) did not pass`);
  if (totals.skipped > 0 || totals.todo > 0) fail("parity tests were skipped: not a full run");
  if (totals.pass !== totals.tests || totals.tests === 0) fail(`the parity run passed ${totals.pass} of ${totals.tests} tests`);
  for (const set of PARITY_SETS) {
    if (!(set in checked)) fail(`the parity run did not report how many ${set} it checked`);
    if (checked[set] === 0) fail(`the parity run checked no ${set}`);
  }
  return { cases: checked.cases, generated: checked["generated issues"] };
}

export const gzipBytes = (path) => {
  const data = readBytes(path);
  if (data.length === 0) fail(`${path} is empty`);
  return gzipSync(data, { level: 9 }).length;
};
const kB = (bytes) => `${(bytes / 1000).toFixed(1)} kB`;

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.replace(/^--/, "");
    if (!key || argv[i + 1] === undefined) fail(`expected --name value pairs, got "${argv.slice(i).join(" ")}"`);
    args[key] = argv[i + 1];
  }
  for (const key of ["out", "tests", "twin", "parity", "wasm"]) if (!args[key]) fail(`--${key} is required`);
  return args;
}

export function buildBadges(args) {
  const tests = parseCargoTest(readLines(args.tests));
  const twin = parseVitest(readLines(args.twin));
  const parity = parseParity(readLines(args.parity));
  return {
    tests: { label: "tests", message: `${tests.passed} passed${tests.ignored ? `, ${tests.ignored} ignored` : ""}`, color: "brightgreen" },
    "twin-tests": { label: "twin tests", message: `${twin.passed} passed${twin.skipped ? `, ${twin.skipped} skipped` : ""}`, color: "brightgreen" },
    parity: { label: "wasm and twin agree on", message: `${parity.cases} cases, ${parity.generated} generated issues`, color: "brightgreen" },
    "wasm-size": { label: "wasm gzip", message: kB(gzipBytes(args.wasm)), color: "blue" },
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    const args = parseArgs(process.argv.slice(2));
    const badges = buildBadges(args);
    mkdirSync(args.out, { recursive: true });
    for (const [name, { label, message, color }] of Object.entries(badges)) {
      const json = `${JSON.stringify({ schemaVersion: 1, label, message, color })}\n`;
      writeFileSync(join(args.out, `${name}.json`), json);
      process.stdout.write(`${name}.json ${json}`);
    }
  } catch (e) {
    if (!(e instanceof BadgeError)) throw e;
    console.error(`badges: ${e.message}`);
    process.exit(1);
  }
}
