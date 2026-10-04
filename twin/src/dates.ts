/*
  Calendar dates as whole days, proleptic Gregorian, no time zone. Integer
  arithmetic only, so the result does not depend on Date parsing rules.
*/

const ISO = /^(\d{4})-(\d{2})-(\d{2})$/;

function isLeap(y: number): boolean {
  return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
}

function daysInMonth(y: number, m: number): number {
  if (m === 2) return isLeap(y) ? 29 : 28;
  return m === 4 || m === 6 || m === 9 || m === 11 ? 30 : 31;
}

/* Days since 1970-01-01 of a YYYY-MM-DD date, null for anything else */
export function parseIsoDate(s: string): number | null {
  const match = typeof s === "string" ? ISO.exec(s) : null;
  if (!match) return null;
  const y = Number(match[1]);
  const m = Number(match[2]);
  const d = Number(match[3]);
  if (m < 1 || m > 12 || d < 1 || d > daysInMonth(y, m)) return null;
  // Count from 0000-03-01 so the leap day ends the year.
  const yy = m <= 2 ? y - 1 : y;
  const mm = m <= 2 ? m + 9 : m - 3;
  const leapDays = Math.floor(yy / 4) - Math.floor(yy / 100) + Math.floor(yy / 400);
  const fromMarch = Math.floor((153 * mm + 2) / 5) + d - 1;
  return 365 * yy + leapDays + fromMarch - 719_468;
}

/* Whole days from one YYYY-MM-DD date to another, null when either is invalid */
export function dayOffset(from: string, to: string): number | null {
  const a = parseIsoDate(from);
  const b = parseIsoDate(to);
  return a === null || b === null ? null : b - a;
}
