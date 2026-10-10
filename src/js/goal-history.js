// Weekly goals change over time; each past week is judged against the goal
// that was in effect then.

/**
 * The goal (minutes) for a week: the latest one recorded on or before it.
 * Weeks older than every record use the oldest record; with no records at
 * all, `fallback` (the current goal) is used.
 * @param {string} weekKey "YYYY-MM-DD" start date of the week
 * @param {{weekKey: string, goalMinutes: number}[]} recorded sorted by weekKey
 */
export function goalForWeek(weekKey, recorded, fallback) {
  let goal = null;
  for (const g of recorded) {
    if (g.weekKey <= weekKey) goal = g.goalMinutes;
  }
  return goal ?? recorded[0]?.goalMinutes ?? fallback;
}
