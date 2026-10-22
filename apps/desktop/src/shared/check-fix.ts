/**
 * FIX UNTIL THE CHECK PASSES (0.812).
 *
 * After a teammate's turn changed files, Locust runs the person's check (A3.3, main/edit-check.ts) and shows what is
 * new in it, with a button to send it back. The harness habit most praised in OpenCode runs the other way: the
 * compiler's word goes straight back to the model, which fixes its own mistake. Aider's --auto-test and a Claude Code
 * Stop hook do the same with a test command. Colin chose the button over an automatic loop on 2026-09-25, because a
 * loop spends turns; so this is the person's choice for each folder, off until they turn it on, and always with a
 * most: "When it fails: Send it back to fix, at most 2". Each fix is one more turn on the teammate's own model.
 *
 * Only after a turn the person sent, never a comparison's column, a routine's step (a routine has its own goal), a
 * reply between teammates or a turn Locust started for another reason; then each fix may lead to the next until the
 * check passes, the fixes run out, or a fix changes nothing in what the check says.
 */
export const MAX_CHECK_FIXES = 5
export const DEFAULT_CHECK_FIXES = 2

/** A folder's most fixes, read defensively: a whole number from 1 to 5, or none. */
export function parsedCheckFixTries(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= MAX_CHECK_FIXES ? value : undefined
}

/** Every folder's most fixes, by workspace id; anything else is left out. */
export function parsedCheckFixes(value: unknown): Readonly<Record<string, number>> | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const kept: Record<string, number> = {}
  for (const [folder, tries] of Object.entries(value as Record<string, unknown>).slice(0, 200)) {
    const most = parsedCheckFixTries(tries)
    if (folder.length > 0 && folder.length <= 200 && most !== undefined) kept[folder] = most
  }
  return Object.keys(kept).length === 0 ? undefined : kept
}

/** What a failed check said, as a fix is told it. */
export interface FailedCheck {
  readonly command: string
  /** What it printed that it did not print before the person's turn: the turn's doing, not what was failing already. */
  readonly lines: readonly string[]
  /** No earlier check in this folder to compare with: every line is listed. */
  readonly first: boolean
}

/**
 * Locust's words to the teammate -- never drawn as the person's. A test changed to pass is the failure a loop like
 * this invites, so it is ruled out, except where the task asked for the behavior the test checks.
 */
export function checkFixPrompt(check: FailedCheck, fix: number, of: number): string {
  const count = `(fix ${String(fix)} of ${String(of)})`
  return [
    fix === 1
      ? `The check \`${check.command}\` failed after your last turn${check.first ? '' : ', with problems it did not have before'} ${count}. What it printed:`
      : `The check \`${check.command}\` still fails after your fix ${count}. What it printed${check.first ? '' : ' that was not failing before your change'}:`,
    '',
    ...(check.lines.length === 0 ? ['    (it printed nothing)'] : check.lines.map((line) => `    ${line}`)),
    '',
    'Fix what your change caused, then say in one or two sentences what you changed. Do not weaken, skip or delete tests, or change the check itself, to make it pass; change a test only where the task asked for the behavior it checks, and say so. Locust runs the check again after your turn.'
  ].join('\n')
}

/** The starter a fix turn is recorded with (ledger v26). */
export interface CheckFixStarter {
  readonly kind: 'check-fix'
  readonly fix: number
  readonly of: number
  readonly command: string
}

/** Why Locust stopped sending it back, in the thread under the turn it stopped after. */
export const CHECK_FIX_STOPPED = {
  outOfFixes: (fixes: number) =>
    `The check still fails after ${String(fixes)} fix${fixes === 1 ? '' : 'es'}, so Locust stopped sending it back. Send it yourself from the card, or raise the most in Settings > Project folder.`,
  noProgress: (fix: number) =>
    `The check still fails the same way after fix ${String(fix)}, so Locust stopped sending it back: another fix would likely end the same way.`,
  /** The turn's own failures are gone; what is left was there before it. */
  fixedWhatItBroke: (fix: number) =>
    `After fix ${String(fix)} the check fails only the way it did before this conversation's change, so Locust stopped there: what is left was failing already.`,
  changedNothing: (fix: number) =>
    `Fix ${String(fix)} changed no files, so the check was not run again and Locust stopped sending it back.`,
  busy: (name: string) => `You were already talking to ${name}, so Locust did not send the failing check back.`,
  couldNotStart: (why: string) => `Locust could not send the failing check back: ${why}`
} as const
