// US federal business days, for the DMCA put-back window (17 U.S.C. 512(g)(2)(C): no less than 10,
// no more than 14 business days after a counter-notice). Weekends and the federal holidays of
// 5 U.S.C. 6103 (with the Saturday->Friday, Sunday->Monday observance rule) are skipped.

const DAY = 86_400_000;

function nthWeekday(year: number, month: number, weekday: number, n: number): number {
  const first = new Date(Date.UTC(year, month, 1)).getUTCDay();
  return 1 + ((weekday - first + 7) % 7) + 7 * (n - 1);
}
function lastWeekday(year: number, month: number, weekday: number): number {
  const days = new Date(Date.UTC(year, month + 1, 0)).getUTCDate();
  const last = new Date(Date.UTC(year, month, days)).getUTCDay();
  return days - ((last - weekday + 7) % 7);
}

/** Observed federal holidays of `year`, as "YYYY-MM-DD" (UTC dates). */
export function federalHolidays(year: number): Set<string> {
  const out = new Set<string>();
  const add = (m: number, d: number) => out.add(new Date(Date.UTC(year, m, d)).toISOString().slice(0, 10));
  const fixed = (m: number, d: number) => {
    const wd = new Date(Date.UTC(year, m, d)).getUTCDay();
    add(m, wd === 6 ? d - 1 : wd === 0 ? d + 1 : d);
  };
  fixed(0, 1); // New Year's Day
  add(0, nthWeekday(year, 0, 1, 3)); // Birthday of Martin Luther King, Jr.
  add(1, nthWeekday(year, 1, 1, 3)); // Washington's Birthday
  add(4, lastWeekday(year, 4, 1)); // Memorial Day
  fixed(5, 19); // Juneteenth
  fixed(6, 4); // Independence Day
  add(8, nthWeekday(year, 8, 1, 1)); // Labor Day
  add(9, nthWeekday(year, 9, 1, 2)); // Columbus Day
  fixed(10, 11); // Veterans Day
  add(10, nthWeekday(year, 10, 4, 4)); // Thanksgiving Day
  fixed(11, 25); // Christmas Day
  // New Year's Day of the next year observed on Dec 31 when it falls on a Saturday.
  if (new Date(Date.UTC(year + 1, 0, 1)).getUTCDay() === 6) add(11, 31);
  return out;
}

export function isBusinessDay(ms: number): boolean {
  const d = new Date(ms);
  const wd = d.getUTCDay();
  if (wd === 0 || wd === 6) return false;
  return !federalHolidays(d.getUTCFullYear()).has(d.toISOString().slice(0, 10));
}

/** The end (23:59:59.999 UTC) of the `n`th business day after the day of `fromMs`. */
export function addBusinessDays(fromMs: number, n: number): number {
  let day = Math.floor(fromMs / DAY) * DAY;
  let left = n;
  while (left > 0) {
    day += DAY;
    if (isBusinessDay(day)) left--;
  }
  return day + DAY - 1;
}

/** The start (00:00 UTC) of the `n`th business day after the day of `fromMs`. */
export function startOfBusinessDay(fromMs: number, n: number): number {
  return addBusinessDays(fromMs, n) - DAY + 1;
}
