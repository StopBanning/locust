import type { ReactElement } from 'react'

/**
 * A change's line counts, without a side that is zero (0.867).
 *
 * Every count in the app was drawn as "+N −M", so a new file read "+299 −0" with the zero in red: on a long Codex
 * turn every step row and every file row carried one, red ink saying nothing (Colin's screenshots of 0.866, the
 * browser open beside a first-visit audit). Claude Code's own app says "+52" for a change that removed nothing, and
 * so does this. A change with neither (a rename, a mode change) draws nothing here; its row still says what it is.
 *
 * The space between the two is a text node: inline it is the gap, and in a flex row it collapses, so a row that
 * spaces its parts with `gap` is spaced as it was.
 */
export function LineCounts({ added, removed }: { readonly added: number; readonly removed: number }): ReactElement {
  return (
    <>
      {added > 0 && <span className="lc-diff__addmark">+{added}</span>}
      {added > 0 && removed > 0 && ' '}
      {removed > 0 && <span className="lc-diff__delmark">−{removed}</span>}
    </>
  )
}
