import { Fragment, useEffect, useMemo, useState } from 'react'
import type { ReactElement } from 'react'

import { colourCode, colouredAlready, languageOfPath } from '../codeColors.js'
import type { ColouredToken } from '../codeColors.js'
import { HUNKS_SHOWN_FIRST, afterText, completenessOf, foldContext, hunkRange, pairedSpans } from '../diff.js'
import { hunkSides, paintRow } from '../diffColours.js'
import type { DiffCounts, DiffFile, DiffHunk, DiffRow, WordSpan } from '../diff.js'
import { diffNoteFor, diffNoteKey, MAX_DIFF_NOTE } from '../diffNotes.js'
import { displayPath } from '../missionView.js'
import { CopyButton } from './CopyButton.js'
import { useDiffNotes } from './DiffNotes.js'
import type { DiffNotesPlace } from './DiffNotes.js'
import { Icon } from './Icon.js'
import { LineCounts } from './LineCounts.js'

/**
 * One file's change, unified, never side by side: the thread column caps at
 * 760px and two panes would leave forty mono characters each.
 *
 * Everything numeric on screen is derived from the rows by `diff.ts` -- the
 * `@@` line, the fold counts, the footer -- so the three numbers a reviewer
 * sees for one change cannot disagree. And the last thing in the box is
 * always a statement of completeness: `All N hunks shown`, an expand button
 * naming exactly what remains, or the remainder the record could not hold.
 * Silence after the last row would read as "that was everything".
 */
export function DiffView({
  file,
  truncated,
  reported,
  workspacePath
}: {
  readonly file: DiffFile
  /** The recorded text is shorter than the change the runtime made. */
  readonly truncated: boolean
  /** What the runtime counted across the whole change, before bounding. */
  readonly reported: DiffCounts | undefined
  readonly workspacePath?: string | undefined
}): ReactElement {
  const [shownHunks, setShownHunks] = useState(Math.min(HUNKS_SHOWN_FIRST, file.hunks.length))
  // A note can be written here only where it can be sent (DiffNotes.tsx).
  const place = useDiffNotes()
  /** The line a note is being written on, by `diffNoteKey`. */
  const [editing, setEditing] = useState<string>()
  const moreParsed = shownHunks < file.hunks.length
  const after = useMemo(() => afterText(file), [file])
  // Parsed hunks are offered before the truncation is confessed: a reader
  // should see every line that was recorded, then be told what was not.
  const completeness = completenessOf(file, shownHunks, truncated && !moreParsed)
  const language = useMemo(() => languageOfPath(file.path), [file.path])
  return (
    <div className="lc-diff" role="region" aria-label={`Changes to ${file.path}`}>
      {file.hunks.slice(0, completeness.shownHunks).map((hunk, index) => (
        <Hunk
          key={`${String(hunk.oldStart)}-${String(hunk.newStart)}-${String(index)}`}
          hunk={hunk}
          language={language}
          path={file.path}
          labelPath={displayPath(file.path, workspacePath)}
          place={place}
          editing={editing}
          onEdit={setEditing}
        />
      ))}
      <div className="lc-diff__foot">
        {/* Copy, as Claude Code offers it (0.649): the code as it reads after the change. */}
        {after !== undefined && <CopyButton className="lc-diff__copy" label={`the new code in ${file.path}`} text={after} />}
        {completeness.canExpand ? (
          <button type="button" className="lc-diff__expand" onClick={() => setShownHunks(file.hunks.length)}>
            {completeness.statement}
          </button>
        ) : (
          <span>
            {completeness.statement}
            {truncated && reported !== undefined && (
              <>
                {' · the runtime counted '}
                <LineCounts added={reported.added} removed={reported.removed} />
                {' in all'}
              </>
            )}
          </span>
        )}
      </div>
    </div>
  )
}

/** The hunk's rows' colours (diffColours.ts), once both sides are coloured; undefined until then, and drawn plain. */
function useHunkColours(hunk: DiffHunk, language: string | undefined): ReadonlyMap<DiffRow, readonly ColouredToken[]> | undefined {
  const sides = useMemo(() => hunkSides(hunk), [hunk])
  const now = (): ReadonlyMap<DiffRow, readonly ColouredToken[]> | undefined => {
    const before = sides.before.length === 0 ? [] : colouredAlready(sides.before, language)
    const after = sides.after.length === 0 ? [] : colouredAlready(sides.after, language)
    return before === undefined || after === undefined ? undefined : rowsOf(sides, before, after)
  }
  const [colours, setColours] = useState(now)
  useEffect(() => {
    if (language === undefined) return
    const already = now()
    setColours(already)
    if (already !== undefined) return
    let shown = true
    const side = (code: string) => (code.length === 0 ? Promise.resolve([]) : colourCode(code, language))
    void Promise.all([side(sides.before), side(sides.after)]).then(([before, after]) => {
      if (shown && before !== undefined && after !== undefined) setColours(rowsOf(sides, before, after))
    })
    return () => {
      shown = false
    }
    // `now` reads only `sides` and `language`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sides, language])
  return colours
}

function rowsOf(
  sides: ReturnType<typeof hunkSides>,
  before: readonly (readonly ColouredToken[])[],
  after: readonly (readonly ColouredToken[])[]
): ReadonlyMap<DiffRow, readonly ColouredToken[]> {
  const rows = new Map<DiffRow, readonly ColouredToken[]>()
  for (const [row, where] of sides.at) {
    const line = (where.side === 'before' ? before : after)[where.line]
    if (line !== undefined) rows.set(row, line)
  }
  return rows
}

function Hunk({
  hunk,
  language,
  path,
  labelPath,
  place,
  editing,
  onEdit
}: {
  readonly hunk: DiffHunk
  readonly language: string | undefined
  readonly path: string
  readonly labelPath: string
  readonly place: DiffNotesPlace | undefined
  readonly editing: string | undefined
  readonly onEdit: (key: string | undefined) => void
}): ReactElement {
  const [openFolds, setOpenFolds] = useState<ReadonlySet<number>>(() => new Set())
  const spans = useMemo(() => pairedSpans(hunk.rows), [hunk])
  const segments = useMemo(() => foldContext(hunk.rows), [hunk])
  const colours = useHunkColours(hunk, language)
  return (
    <>
      <div className="lc-diff__hunk">
        <span>{hunkRange(hunk)}</span>
        {hunk.heading.length > 0 && <span>{hunk.heading}</span>}
      </div>
      {segments.map((segment, index) => {
        if (segment.kind === 'rows' || openFolds.has(index)) {
          return segment.rows.map((row, rowIndex) => (
            <Row
              key={`${String(index)}-${String(rowIndex)}`}
              row={row}
              spans={spans.get(row)}
              tokens={colours?.get(row)}
              path={path}
              labelPath={labelPath}
              place={place}
              editing={editing}
              onEdit={onEdit}
            />
          ))
        }
        return (
          <button
            key={`fold-${String(index)}`}
            type="button"
            className="lc-diff__fold"
            onClick={() => setOpenFolds(new Set([...openFolds, index]))}
          >
            <span className="lc-diff__foldmark" aria-hidden="true">
              ⋯
            </span>
            <span>
              {segment.count} unchanged line{segment.count === 1 ? '' : 's'}
            </span>
          </button>
        )
      })}
    </>
  )
}

/**
 * The sign column carries the same fact as the row fill, so the diff reads
 * without colour. Gutters are unselectable: a copy of the diff yields code.
 */
function Row({
  row,
  spans,
  tokens,
  path,
  labelPath,
  place,
  editing,
  onEdit
}: {
  readonly row: DiffRow
  readonly spans: readonly WordSpan[] | undefined
  readonly tokens: readonly ColouredToken[] | undefined
  readonly path: string
  readonly labelPath: string
  readonly place: DiffNotesPlace | undefined
  readonly editing: string | undefined
  readonly onEdit: (key: string | undefined) => void
}): ReactElement {
  const key = diffNoteKey(path, row)
  const note = place?.notes.find((entry) => entry.key === key)
  const where = row.kind === 'del' ? `removed line ${String(row.oldNo ?? '')}` : `line ${String(row.newNo ?? row.oldNo ?? '')}`
  return (
    <>
      <div className={`lc-diff__row is-${row.kind}${note === undefined ? '' : ' has-note'}`}>
        {/*
          * The "+" is on the line it is about, over its numbers, and only on
          * hover or focus -- a column of buttons down every diff would be the
          * loudest thing in the thread (GitHub's gutter, Orca's review).
          */}
        {place !== undefined && editing !== key && (
          <button
            type="button"
            className="lc-diff__noteadd"
            aria-label={`Add a note on ${labelPath}, ${where}`}
            title={`Add a note for ${place.who} on this line`}
            onClick={() => onEdit(key)}
          >
            <Icon name="plus" size={11} />
          </button>
        )}
        <DiffRowCells row={row} spans={spans} tokens={tokens} />
      </div>
      {note !== undefined && editing !== key && place !== undefined && (
        <div className="lc-diff__note">
          <span className="lc-diff__notetext">{note.text}</span>
          <button type="button" className="lc-linkbutton" onClick={() => onEdit(key)}>
            Edit
          </button>
          <button type="button" className="lc-linkbutton" aria-label={`Remove the note on ${where}`} onClick={() => place.remove(key)}>
            Remove
          </button>
        </div>
      )}
      {editing === key && place !== undefined && (
        <NoteEditor
          initial={note?.text ?? ''}
          who={place.who}
          onSave={(text) => {
            if (text.trim().length === 0) place.remove(key)
            else place.add(diffNoteFor(path, row, text))
            onEdit(undefined)
          }}
          onCancel={() => onEdit(undefined)}
        />
      )}
    </>
  )
}

/**
 * The note being written, under its line. Enter keeps it; Shift+Enter is a
 * new line; Escape puts it away; keeping it empty takes the note off.
 */
function NoteEditor({
  initial,
  who,
  onSave,
  onCancel
}: {
  readonly initial: string
  readonly who: string
  readonly onSave: (text: string) => void
  readonly onCancel: () => void
}): ReactElement {
  const [text, setText] = useState(initial)
  return (
    <form
      className="lc-diff__noteedit"
      onSubmit={(event) => {
        event.preventDefault()
        onSave(text)
      }}
    >
      <textarea
        className="lc-input"
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing || event.keyCode === 229) return
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault()
            onSave(text)
          }
          if (event.key === 'Escape') onCancel()
        }}
        placeholder={`A note for ${who} on this line. It goes with your next message.`}
        aria-label="Note on this line"
        rows={2}
        maxLength={MAX_DIFF_NOTE}
        autoFocus
      />
      <div className="lc-diff__noteactions">
        <button type="submit" className="lc-button is-active">
          {initial.length === 0 ? 'Add note' : 'Save note'}
        </button>
        <button type="button" className="lc-ghostbutton" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  )
}

function DiffRowCells({ row, spans, tokens }: { readonly row: DiffRow; readonly spans: readonly WordSpan[] | undefined; readonly tokens?: readonly ColouredToken[] | undefined }): ReactElement {
  return (
    <>
      <span className="lc-diff__no" aria-hidden="true">
        {row.oldNo ?? ''}
      </span>
      <span className="lc-diff__no" aria-hidden="true">
        {row.newNo ?? ''}
      </span>
      <span className="lc-diff__sign">{row.kind === 'add' ? '+' : row.kind === 'del' ? '−' : ''}</span>
      <span className="lc-diff__code">
        {spans === undefined && tokens === undefined
          ? row.text
          : paintRow(row.text, tokens, spans).map((run, index) => {
              const coloured = run.color === undefined ? run.text : <span style={{ color: run.color }}>{run.text}</span>
              return run.changed ? (
                <mark key={String(index)} className="lc-diff__word">
                  {coloured}
                </mark>
              ) : (
                <Fragment key={String(index)}>{coloured}</Fragment>
              )
            })}
      </span>
    </>
  )
}
