/**
 * THE EXACT ACTION, WHOLE (QA-2026-09-29 round 2, R14).
 *
 * An approval card is headed "exact action", and it showed the first 600
 * characters of a Claude tool call's input (4,000 of a Codex command), cut
 * mid-word, flattened to one line, with nothing counting what was left out --
 * while Approve let the whole of it through. A mail's recipient written last
 * in its input appeared nowhere on the card, and the field order is the
 * model's to choose, so text it read can put the part that matters past the
 * cut.
 *
 * So the whole of it is shown: line breaks kept (the field wraps and scrolls),
 * a connector's input set out one field to a line. Past a size no real call
 * reaches, the rest is COUNTED, never silently dropped.
 */
export const MAX_APPROVAL_DETAIL = 20_000

export function wholeDetail(text: string): string {
  const kept = text.replace(/\r\n?/g, '\n').trim()
  if (kept.length <= MAX_APPROVAL_DETAIL) return kept
  const rest = kept.length - MAX_APPROVAL_DETAIL
  return `${kept.slice(0, MAX_APPROVAL_DETAIL)}\n\n[${rest.toLocaleString('en-US')} more character${rest === 1 ? '' : 's'} not shown here. Deny if you cannot see enough to decide.]`
}

/**
 * THE PROGRAM AS THE SHELL GETS IT (0.837). Codex on Windows sends a command
 * with its program's path escaped, every backslash doubled:
 * `"C:\\Windows\\System32\\...\\powershell.exe" -Command 'python -m pytest -q'`.
 * The card and the Board printed it so -- an "exact action" that is not what
 * runs (site frames, 2026-10-27). When every backslash in the leading quoted
 * path is half of a pair, each pair is read as one; anything else, and the
 * rest of the command, is shown exactly as sent.
 */
export function programAsRun(command: string): string {
  const leading = /^"([^"]*)"/.exec(command)
  const path = leading?.[1]
  if (path === undefined || !path.includes('\\\\') || path.replace(/\\\\/g, '').includes('\\')) return command
  return `"${path.replace(/\\\\/g, '\\')}"${command.slice(path.length + 2)}`
}

/** A tool call's input, one field to a line, so every field is on the card. */
export function wholeInput(input: unknown): string {
  try {
    return wholeDetail(JSON.stringify(input, null, 2) ?? '')
  } catch {
    return ''
  }
}
