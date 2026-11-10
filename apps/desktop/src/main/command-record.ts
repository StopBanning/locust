import type { RuntimeCommandSpec } from '@teammate/runtime-adapters'

/**
 * What the host actually ran, in a form safe to keep and safe to send.
 *
 * Chasing why every Copilot mission failed on 2026-09-05 took eight
 * experiments, and every one of them was reconstructing the command by
 * reading `commands.ts` and hoping the reconstruction matched. It would have
 * taken one if the record had said. That is a diagnostic gap in normal times
 * and a blocking one for a beta: a tester's machine cannot be borrowed, and
 * "it failed" without the argv is a question nobody can answer remotely.
 *
 * Two rules make it safe to keep.
 *
 * The PROMPT NEVER APPEARS. Codex and Claude take it on stdin, but OpenCode
 * and Copilot take it as an argument, so a naive argv would copy the person's
 * words into the header of every mission -- doubling them in the file and,
 * worse, putting them somewhere a "send me your ledger" request would carry
 * them. The ledger already holds the prompt once, deliberately and in one
 * place. Here it is replaced by a marker, matched by VALUE rather than by
 * position, because the flag it follows differs per runtime.
 *
 * And it is BOUNDED. A record that can grow with its input is a record that
 * can be used to bloat the file it lives in.
 */

/** Longer than any real flag; a value past this is truncated, never dropped silently. */
const MAX_ARGUMENT_LENGTH = 512
/** More than any builder here produces, with room for a runtime that grows one. */
const MAX_ARGUMENTS = 64

export interface RecordedCommand {
  readonly cwd: string
  readonly executablePath: string
  /** The arguments as run, with the prompt replaced by `<prompt>`. */
  readonly args: readonly string[]
}

/**
 * The command as it will be recorded. `prompt` is what the host is about to
 * send, so it can be found in the argv whatever flag introduced it.
 */
export function recordableCommand(spec: RuntimeCommandSpec, prompt: string): RecordedCommand {
  const trimmed = prompt.trim()
  const args: string[] = []
  for (const argument of spec.args.slice(0, MAX_ARGUMENTS)) {
    // Compared whole, not by inclusion: a flag that happens to contain the
    // prompt as a substring is not the prompt, and a prompt that happens to
    // equal a flag is still the prompt. Whole-value equality is the only
    // comparison that is right in both directions.
    if (trimmed.length > 0 && argument.trim() === trimmed) {
      args.push('<prompt>')
      continue
    }
    args.push(
      argument.length <= MAX_ARGUMENT_LENGTH
        ? argument
        : `${argument.slice(0, MAX_ARGUMENT_LENGTH - 1)}…`
    )
  }
  if (spec.args.length > MAX_ARGUMENTS) {
    args.push(`… ${String(spec.args.length - MAX_ARGUMENTS)} more`)
  }
  return { executablePath: spec.executablePath, args, cwd: spec.cwd }
}
