// Deep comparison with the tolerance every parity check in this repository
// uses: numbers agree when |a - b| <= 1e-6 * max(|a|, |b|) (so a zero must
// match exactly), both NaN counts as equal, and NaN may be written "NaN".
// Typed arrays compare as arrays; strings, booleans and null exactly;
// objects must have the same keys.

export const RELATIVE = 1e-6;

const isNumberLike = (v) => typeof v === "number" || v === "NaN";
const toNumber = (v) => (v === "NaN" ? Number.NaN : v);
const isArrayLike = (v) => Array.isArray(v) || ArrayBuffer.isView(v);

/** Replaces every "NaN" string in a JSON value with NaN. */
export function decodeNaN(v) {
  if (v === "NaN") return Number.NaN;
  if (Array.isArray(v)) return v.map(decodeNaN);
  if (v && typeof v === "object") {
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, decodeNaN(x)]));
  }
  return v;
}

/**
 * Compares got with want; returns the failures and the largest relative
 * difference among numbers that agree but are not identical.
 */
export function compare(got, want, path = "", report = { failures: [], worst: 0, worstAt: "" }) {
  if (isNumberLike(got) && isNumberLike(want)) {
    const g = toNumber(got);
    const w = toNumber(want);
    if (Number.isNaN(g) && Number.isNaN(w)) return report;
    if (g === w) return report;
    const diff = Math.abs(g - w);
    const scale = Math.max(Math.abs(g), Math.abs(w));
    if (!(diff <= RELATIVE * scale)) {
      report.failures.push(`${path}: got ${g}, want ${w}`);
    } else if (diff / scale > report.worst) {
      report.worst = diff / scale;
      report.worstAt = path;
    }
    return report;
  }
  if (isArrayLike(got) && isArrayLike(want)) {
    if (got.length !== want.length) {
      report.failures.push(`${path}: length ${got.length} vs ${want.length}`);
      return report;
    }
    for (let i = 0; i < want.length; i++) compare(got[i], want[i], `${path}[${i}]`, report);
    return report;
  }
  if (got && want && typeof got === "object" && typeof want === "object") {
    const gk = Object.keys(got).sort();
    const wk = Object.keys(want).sort();
    if (gk.join() !== wk.join()) {
      report.failures.push(`${path}: keys ${gk.join(",")} vs ${wk.join(",")}`);
      return report;
    }
    for (const k of wk) compare(got[k], want[k], `${path}.${k}`, report);
    return report;
  }
  if (got !== want) report.failures.push(`${path}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
  return report;
}
