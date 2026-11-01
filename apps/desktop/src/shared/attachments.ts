/**
 * Files a person attached to a message, said in a way every runtime can act on.
 *
 * Measured 2026-09-08 (`docs/ATTACHMENTS-INTAKE-2026-09-08.md`): of the six
 * runtimes, three take a file natively (`opencode -f`, `copilot
 * --attachment`, `codex -i` for images) and three take nothing at all. What
 * ALL six can do is read a file inside the workspace, because that is what
 * they are for.
 *
 * So the first version attaches by REFERENCE: the message names the paths and
 * asks the runtime to read them. That is not a stand-in for the real thing --
 * the file genuinely reaches the model, by the runtime's own read, and the
 * read then appears as a tool row in the activity fold. The app can show that
 * it happened instead of claiming it did.
 *
 * Two rules this module exists to keep:
 *
 *   NEVER INLINE THE CONTENTS. A 40 KB paste would eat the prompt budget that
 *   inbound peer messages already lose to (`composeRuntimePrompt` drops a
 *   waiting message when the prompt runs long, measured in its own tests).
 *   A path costs a line.
 *
 *   NEVER CLAIM MORE THAN HAPPENED. The line says "read these files", because
 *   that is what was asked. When step B hands the three capable runtimes the
 *   file natively, the wording for those changes to match.
 */

/** The most files one message may carry. */
export const MAX_ATTACHMENTS = 8

/**
 * The most one pasted file may weigh: it crosses IPC in one message. Generous for a screenshot -- a full 4K PNG is well
 * under it -- and short of anything that would stall a window. A browser's chosen files go the same way (0.850), so it
 * says so before sending.
 */
export const MAX_PASTED_BYTES = 24 * 1024 * 1024

/**
 * The line that goes above the person's own words.
 *
 * Above, not below: the runtime should know what it is working from before it
 * reads the request. Returns undefined for no attachments, so a caller cannot
 * accidentally prepend an empty instruction.
 */
export function attachmentPreamble(paths: readonly string[]): string | undefined {
  if (paths.length === 0) return undefined
  // A name holding a line break (legal on macOS and Linux) wrote a line of
  // its own into the brief (QA-2026-09-29 round 2, R20). Such a name is
  // quoted, its breaks escaped, so it stays one item and says exactly what
  // the file is called.
  const listed = paths.map((path) => `- ${/[\u0000-\u001f\u007f]/.test(path) ? JSON.stringify(path) : path}`).join('\n')
  return paths.length === 1
    ? `Read this file in the workspace before you answer:\n${listed}`
    : `Read these ${String(paths.length)} files in the workspace before you answer:\n${listed}`
}

/**
 * The prompt actually sent: the preamble, a blank line, then what was typed.
 *
 * `note`, when given, goes between the two: what Locust did to make the files
 * readable (`readablePdfNote`). It is said only to the runtime -- the
 * recorded prompt never carries one, so `splitAttachments` never meets it.
 */
export function withAttachments(prompt: string, paths: readonly string[], note?: string): string {
  const preamble = attachmentPreamble(paths)
  if (preamble === undefined) return prompt
  return note === undefined || note.length === 0 ? `${preamble}\n\n${prompt}` : `${preamble}\n\n${note}\n\n${prompt}`
}

/**
 * A PDF, opened for the agent (0.713).
 *
 * What the host made of one attached PDF: its text, and a picture of each of
 * its first pages, written into Locust's own folder (main/pdf-reading.ts).
 * Folder-relative paths, forward slashes.
 */
export interface ReadablePdf {
  /** The attachment, as the message names it. */
  readonly pdf: string
  /** Its text, one file; absent when no page has any (a scan). */
  readonly text?: string
  /** One picture per page, first page first. */
  readonly pictures: readonly string[]
  readonly pages: number
}

/**
 * What the runtime is told about the PDFs Locust opened, or undefined when
 * there are none.
 *
 * A tester's homework, 0.712: Codex had no PDF tool that worked on his
 * machine and spent eight commands (four failed) decoding the file by hand.
 * The sentence says plainly that no tool is needed, and which file holds
 * what -- the text for reading, the pictures for what text cannot carry
 * (an equation, a table, a chart), because a matrix pulled out of a PDF as
 * text is a row of numbers with its brackets read as digits.
 */
export function readablePdfNote(readables: readonly ReadablePdf[]): string | undefined {
  if (readables.length === 0) return undefined
  const lines = readables.map((readable) => {
    const parts: string[] = []
    if (readable.text !== undefined) parts.push(`its text is in ${readable.text}`)
    else parts.push('it has no text in it (scanned pages), so read the pictures')
    if (readable.pictures.length > 0) {
      const first = readable.pictures[0]!
      const last = readable.pictures[readable.pictures.length - 1]!
      const which =
        readable.pictures.length === readable.pages
          ? readable.pages === 1
            ? 'its page is a picture'
            : `each of its ${String(readable.pages)} pages is a picture`
          : `its first ${String(readable.pictures.length)} of ${String(readable.pages)} pages are pictures`
      parts.push(readable.pictures.length === 1 ? `${which}: ${first}` : `${which}, ${first} to ${last.slice(last.lastIndexOf('/') + 1)}`)
    }
    return `- ${readable.pdf}: ${parts.join('; ')}`
  })
  return [
    readables.length === 1
      ? 'Locust has already opened the PDF for you, so no PDF tool or package is needed:'
      : `Locust has already opened the ${String(readables.length)} PDFs for you, so no PDF tool or package is needed:`,
    ...lines,
    'Read the text first. For equations, tables, charts and layout, look at the pictures: they show the page as printed, and the text may not.'
  ].join('\n')
}

/**
 * What the composer chip says.
 *
 * Deliberately "1 file" rather than "attached": on five of six runtimes
 * nothing is attached in the API sense, and a word that overstates what
 * happened is the thing this app keeps refusing to do.
 */
export function attachmentLabel(count: number): string {
  return count === 1 ? '1 file' : `${String(count)} files`
}

/** What was typed, and what was attached, recovered from a sent prompt. */
export interface SplitPrompt {
  /** The person's own words, without the line the host added. */
  readonly text: string
  /** Workspace-relative paths, in the order they were attached. */
  readonly attachments: readonly string[]
}

/**
 * The inverse of `withAttachments`, for everything that SHOWS a prompt.
 *
 * The preamble is written for the runtime, and it was reaching the person as
 * well: the bubble in their own thread, the sidebar row, the title a mission
 * is found by. All three read "Read this file in the workspace before you
 * answer: - NOTES.md" above words they had actually typed, which is the app
 * putting its own machinery in their mouth.
 *
 * Recovering the pieces here rather than carrying them alongside the prompt is
 * deliberate: every mission already saved has the preamble inside its recorded
 * prompt, and a field added today would leave all of those still reading the
 * instruction. This fixes them too.
 *
 * It matches ONLY what `attachmentPreamble` writes -- the exact opening, a
 * list of `- ` lines, a blank line, then the rest. Anything else comes back
 * whole, because a prompt this cannot account for is a prompt to leave alone.
 */
export function splitAttachments(prompt: string): SplitPrompt {
  const match = /^Read th(?:is file|ese \d+ files) in the workspace before you answer:\n((?:- [^\n]+\n?)+)\n([\s\S]+)$/.exec(prompt)
  if (match === null) return { text: prompt, attachments: [] }
  const listed = (match[1] ?? '')
    .split('\n')
    .filter((line) => line.startsWith('- '))
    .map((line) => line.slice(2))
  // A preamble with nothing under it is not one of ours: `withAttachments`
  // only ever writes the line above something the person typed.
  const rest = match[2] ?? ''
  if (listed.length === 0 || rest.length === 0) return { text: prompt, attachments: [] }
  return { text: rest, attachments: listed }
}

/**
 * Where a file copied in from outside the workspace lives, workspace-relative.
 *
 * Shared rather than main-only because the composer's tile says so too: the
 * "copied in" fact belongs on the durable object, and its title names this
 * folder. The main process owns the copying; both sides name the same place.
 */
export const ATTACHMENT_DIR = '.locust/attachments'
