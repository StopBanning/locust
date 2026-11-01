import { deskAccess, WINDOW_ONLY_SAYS } from '../../shared/desk-calls.js'

/*
 * ON THE COMPUTER ONLY (0.849). Locust in a browser refuses the calls that stay on the computer (desk-calls.ts,
 * WINDOW_ONLY), and the window found out only after a press: Colin, in Safari on app.locust.lol (2026-11-01), met
 * Discord, Slack and Telegram each saying "This setting could not be read", voice typing "could not be saved", a Plans
 * switch that sprang back, and "Allow it in GitHub" doing nothing at all. A control now asks before it is drawn.
 */

/** Whether this window is Locust in a browser (desk-remote.ts gives it `remote`); never on the computer or in a test. */
export const inABrowser = (): boolean => typeof window !== 'undefined' && window.desktop?.remote !== undefined

/** The words a control says in place of working, where this call stays on the computer; undefined where it works. */
export function onTheComputer(channel: string, args: readonly unknown[] = []): string | undefined {
  return inABrowser() && deskAccess(channel, args) === undefined ? WINDOW_ONLY_SAYS : undefined
}

/** A call's failure in words: the refusal's own where a browser was refused, else the row's own. */
export function failureSaid(error: unknown, own: string): string {
  return error instanceof Error && error.message === WINDOW_ONLY_SAYS ? WINDOW_ONLY_SAYS : own
}
