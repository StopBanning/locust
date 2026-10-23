/**
 * Every tag this app parses out of a reply, and the one rule for text that
 * did not come from the person.
 *
 * Locust reads five protocol blocks out of what a teammate says, and each one
 * makes the host DO something: `locust-share` sends a message as that
 * teammate, `locust-memory` writes to the team's memory, `locust-ask` puts a
 * decision in front of Colin, `locust-task` moves a room's board, and
 * `locust-file` hands the person a file the teammate wrote.
 *
 * Model-authored text gets re-injected into other prompts all over this app --
 * a peer message quoted into the recipient's briefing, a reviewed turn quoted
 * into the reviewer's, a relayed answer handed to the next teammate. Any of
 * those quotes can carry a block, and a model asked to review or reply to
 * quoted text very often reproduces it verbatim. The block then arrives in the
 * SECOND teammate's reply, where the host parses it and acts -- under the
 * second teammate's name, with nobody having asked for it.
 *
 * `peer-share.ts` has defended exactly this since peer messages existed, and
 * its comment says why. What it did not do was cover the other three tags, or
 * the places other than a peer message where model text is quoted. Found
 * 2026-09-13 reading xai-org/grok-build, whose harness tags every input with
 * an `InputAuthority` -- `HumanIntent`, `ModelAuthoredUntrusted`,
 * `RuntimeControl` -- and lets what an input is ALLOWED to do follow from
 * that. This is the small honest version of the same idea: one rule, in one
 * place, for text that is not the person's.
 *
 * Defanged rather than deleted. The reader still needs to see what was said --
 * that IS the point of quoting it -- so the angle bracket becomes a visibly
 * different character and the words survive intact.
 */
// `locust-instructions` (0.831, shared/interview.ts): a teammate's own standing instructions, written in an interview.
export const PROTOCOL_TAGS = ['locust-share', 'locust-memory', 'locust-ask', 'locust-task', 'locust-file', 'locust-tidy', 'locust-instructions'] as const

/** The mark a defanged bracket leaves. Single-width, visibly not a bracket. */
export const DEFANGED_BRACKET = '‹'

const ANY_TAG = new RegExp(`<(/?)(${PROTOCOL_TAGS.join('|')})\\b`, 'gi')

/**
 * Text from a model, made safe to quote into another model's prompt.
 *
 * Call this at every point where something a teammate SAID becomes part of
 * what another teammate is TOLD. If a new protocol tag is added, add it to
 * `PROTOCOL_TAGS` and every one of those points is covered at once -- which
 * is the whole reason this is not four regexes in four files.
 */
export function defangProtocolBlocks(text: string): string {
  return text.replace(ANY_TAG, `${DEFANGED_BRACKET}$1$2`)
}

/**
 * A protocol tag in its loosest possible form: the name, anything that is not
 * another tag, a close bracket. Deliberately looser than any of the five
 * parsers.
 */
// `\\b`, not `\b`: inside a template literal `\b` is the BACKSPACE character,
// not a word boundary, and the pattern then matches nothing at all — silently,
// because it is still a perfectly valid regex.
const LOOSE_TAG = new RegExp(`</?(?:${PROTOCOL_TAGS.join('|')})\\b[^<>]{0,400}>`, 'gi')

/**
 * A tag the run was CUT OFF inside, at the very end of the text.
 *
 * Colin, 2026-09-20, with a bubble ending `</locust-` on a turn whose own card
 * read "The run could not continue -- recovered as interrupted": *"happened
 * before update, could be fixed but i doubt"*. It can.
 *
 * `LOOSE_TAG` requires a closing bracket and a whole tag name, because that is
 * what makes it safe to delete anything at all. An interrupted turn produces
 * neither: the stream stops mid-word, so what reaches the person is the first
 * few characters of a tag and nothing else. Every complete tag on the message
 * came off and this one stayed, which reads as the teammate having typed it.
 *
 * NARROW ON PURPOSE, three ways. It matches only at the END of the text,
 * because that is the only place a cut-off stream can leave one; only a
 * PREFIX of a tag this app actually knows; and only from four characters in
 * (`<locu`, `</locu`), so a lone `<` or a truncated `</li` from some other
 * language is left exactly where it is.
 *
 * `[a-z-]*` after the prefix, not `.*`: the partial cannot contain a space or
 * a bracket, or it is not a cut-off tag name -- it is prose with a bracket in
 * it, and prose is the thing this function exists to protect.
 */
const TORN_TAG = new RegExp(
  // Escaped for the TEMPLATE, then for the regex: a lone backslash-s in a
  // template literal is just `s`, so this stripped trailing s's -- "yes"
  // before a torn tag came back "ye" (L7, the code review).
  `\\n?\\s*</?(?:${PROTOCOL_TAGS.map((tag) => tag.slice(0, 4)).join('|')})[a-z-]*$`,
  'i'
)

/** Fenced blocks and inline spans, which are quoted source and stay intact. */
const CODE = /```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`/g

/**
 * The blocks a host may ACT on: every match of `block` whose opening tag is
 * not inside code.
 *
 * The display has always left code alone ("CODE IS LEFT ALONE", below): a
 * teammate explaining the protocol writes these tags on purpose, in a fence,
 * and that is the answer, not plumbing. The parsers read the raw text, so the
 * same example could send a message, keep a memory, move a task, ask the
 * person, or hand a file (harness review, 2026-09-24).
 *
 * Only the OPENING TAG is tested, never the body: a real block often quotes
 * code -- "Tests run with `pnpm test`" -- and blanking code out of the text
 * would have cut the command out of the memory. Measured on Colin's ledger
 * that day: 27 share blocks and 68 memory blocks outside code, and the 3
 * inside code all describing the protocol; no real block was ever fenced.
 */
export function blocksOutsideCode(text: string, block: RegExp): RegExpMatchArray[] {
  const spans = [...text.matchAll(CODE)].map((span) => [span.index ?? 0, (span.index ?? 0) + span[0].length] as const)
  const inCode = (at: number): boolean => spans.some(([from, to]) => at >= from && at < to)
  /*
   * AN OPENING TAG IN CODE DOES NOT SWALLOW THE REAL BLOCK AFTER IT (0.512).
   * Colin, 2026-09-30, a reply drawn as "Yes. I ended with a `": Ghost wrote
   * "I ended with a `<locust-file>` block", then the real block. `matchAll`
   * matched from the mention in backticks to the real block's close, this
   * dropped that match for starting in code -- and the real block, inside it,
   * was never matched on its own: no file button, and every stripper built on
   * the same pattern deleted the words in between. A match that starts in
   * code is skipped by ONE character, so the search finds the next opening.
   */
  const pattern = new RegExp(block.source, block.flags.includes('g') ? block.flags : `${block.flags}g`)
  const found: RegExpMatchArray[] = []
  for (let match = pattern.exec(text); match !== null; match = pattern.exec(text)) {
    if (inCode(match.index)) {
      pattern.lastIndex = match.index + 1
      continue
    }
    found.push(match)
    if (match[0].length === 0) pattern.lastIndex += 1
  }
  return found
}

/**
 * The text without the blocks `blocksOutsideCode` finds -- and nothing else.
 * Every stripper uses this (0.512): `text.replace(BLOCK, '')` started at an
 * opening tag quoted in code and deleted everything up to the next close.
 */
export function stripBlocksOutsideCode(text: string, block: RegExp): string {
  let kept = ''
  let from = 0
  for (const match of blocksOutsideCode(text, block)) {
    const at = match.index ?? 0
    kept += text.slice(from, at)
    from = at + match[0].length
  }
  return kept + text.slice(from)
}

/**
 * The last thing done to a reply before a person reads it: take the tags off
 * anything still wearing them.
 *
 * WHY THIS EXISTS. Each of the five parsers is strict, and each has a matching
 * stripper built from the SAME strict pattern -- so a block the parser will
 * not act on is also a block the stripper will not hide. Those two facts
 * multiply into the worst possible pair: the message goes nowhere AND the
 * person is shown the plumbing.
 *
 * Colin, 2026-09-20, with a photograph of a bubble reading
 * `<locust-share>Booty :: Reply with exactly the word PEBBLE-6725...`: "bug i
 * think". A bare open tag -- no `to=` -- which `parseShareBlocks` drops and
 * the share stripper's `\s+` never matched. The same hole is in the other
 * four from the opposite side: they accept no attributes at all, so
 * `<locust-memory scope="team">` leaks exactly as loudly.
 *
 * UNWRAP, DO NOT DELETE. The strippers delete because their block was ACTED
 * ON -- it is already a peer card, a memory entry, a decision. Nothing acted
 * on this one, so its body is the only copy of what the teammate said, and
 * deleting it would trade a visible defect for a silent one. The tags come
 * off; the words stay, as ordinary prose the person can read and forward.
 *
 * Unpaired tags are handled by not caring about pairs. A turn cut off
 * mid-block leaves an open tag with no close, which is the case a
 * block-shaped pattern gets wrong and this gets right for free.
 *
 * CODE IS LEFT ALONE. A teammate reading this repo from inside Locust and
 * explaining the protocol writes these tags ON PURPOSE, in a fence, and that
 * is not plumbing -- it is the answer. It happens here often enough to be in
 * these comments twice already.
 */
export function unwrapProtocolTags(text: string): string {
  let out = ''
  let at = 0
  for (const span of text.matchAll(CODE)) {
    const index = span.index ?? 0
    out += text.slice(at, index).replace(LOOSE_TAG, '')
    out += span[0]
    at = index + span[0].length
  }
  out += text.slice(at).replace(LOOSE_TAG, '')
  /*
   * The torn one last, and only on the tail: it is anchored to the end of the
   * string, so it can only be judged once every complete tag is gone.
   */
  out = out.replace(TORN_TAG, '')
  /*
   * Leading blank LINES, not leading whitespace. A tag on its own first line
   * leaves the bubble starting on an empty one, which reads as a gap the
   * teammate left. `trim()` would also eat the indentation of a message that
   * opens with an indented code block, and that indentation is content.
   */
  return out
    .replace(/\n{3,}/g, '\n\n')
    .replace(/^\n+/, '')
    .trimEnd()
}
