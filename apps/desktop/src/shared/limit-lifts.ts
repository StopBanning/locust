/*
 * A LIMIT LIFTS WHEN THE ACCOUNT SAYS SO (0.878; Colin, 17 Nov). Colin switched Antigravity accounts and every
 * Antigravity model stayed AT LIMIT: the tag came off only when a run on it finished in Locust, and his runs were in
 * Antigravity's own app, so Locust never saw one. Two things now lift it without a run: Antigravity's own usage
 * reading for the account signed in now (read when the model list is, including each time the picker opens) saying
 * every window it names has some left, and the reset the limit named having passed.
 */

/** Antigravity's live reading (`antigravityUsageText`) names at least one window, and every window has some left. */
export function readingHasQuotaLeft(said: string): boolean {
  const left = [...said.matchAll(/(\d+)% left/gu)].map((match) => Number(match[1]))
  return left.length > 0 && left.every((percent) => percent > 0)
}

/** The limit named a reset time Locust can read (`resets <ISO time>`), and it has passed. */
export function limitHasReset(said: string, now: number): boolean {
  const reset = /resets (\S+)/u.exec(said)?.[1]
  const at = reset === undefined ? Number.NaN : Date.parse(reset)
  return at <= now // a time it cannot read is NaN, never passed
}
