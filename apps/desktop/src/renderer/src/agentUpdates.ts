import type { RuntimeUpdateView } from '../../shared/ipc.js'

/**
 * What a runtime's row in Settings says about keeping it current -- one line,
 * and none at all when it is current: a row only speaks when its state is not
 * the ordinary one (main/runtime-updates.ts has the rules).
 */
export function updateLine(view: RuntimeUpdateView | undefined, now: Date = new Date()): string | undefined {
  if (view === undefined) return undefined
  const status = view.status
  switch (status.kind) {
    case 'current':
      return undefined
    case 'waiting':
      return status.why === 'ask'
        ? `${status.version} is out.`
        : status.why === 'in use'
          ? `${status.version} is out. It can update once nothing is using it.`
          : status.why === 'held'
            ? `${status.version} is out, and held back: Locust's check of new versions could not read it yet. Update to try it anyway.`
            : `${status.version} is out. It updates once it has been out 12 hours.`
    case 'updating':
      return `Updating to ${status.version}…`
    case 'updated':
      return `Updated from ${status.from} to ${status.to} ${when(status.at, now)}.`
    case 'failed':
      return `Could not update to ${status.version} ${when(status.at, now)}: ${status.what}`
  }
}

/** "at 4:42 PM" today, "on Sep 22" before. */
function when(at: string, now: Date): string {
  const then = new Date(at)
  if (Number.isNaN(then.getTime())) return ''
  return then.toDateString() === now.toDateString()
    ? `at ${then.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit', hour12: true })}`
    : `on ${then.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}`
}

/** Whether a row offers Update: a newer version is out and waiting on the person, or on nothing using it. */
export function offersUpdate(view: RuntimeUpdateView | undefined): boolean {
  return view !== undefined && view.status.kind === 'waiting' && view.status.why !== 'too new'
}

/**
 * The switch's own sentence, saying what an update costs either way: Codex
 * CLI's was 159 MB. On by default (0.304; 0.303 had it off, suspecting the
 * download for a beta tester's dropped connection -- the drops went on, only
 * on Codex runs).
 */
export function keepCurrentNote(automatic: boolean): string {
  return automatic
    ? 'Updating on their own. A newer Codex CLI or Copilot CLI is downloaded when it is out and nothing is using it — a big download, which slows the connection while it runs.'
    : 'Updating when you press Update. A newer Codex CLI or Copilot CLI is shown on its row, and the download — Codex CLI\'s is about 160 MB — starts only when you ask.'
}
