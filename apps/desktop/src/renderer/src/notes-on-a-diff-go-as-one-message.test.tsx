import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { DiffNotesContext } from './components/DiffNotes.js'
import type { DiffNotesPlace } from './components/DiffNotes.js'
import { DiffView } from './components/DiffView.js'
import { parseUnifiedDiff } from './diff.js'
import { diffNoteFor, diffNoteKey, diffNotesBlock, diffNotesTile, withDiffNotes, withNote } from './diffNotes.js'
import type { DiffNote } from './diffNotes.js'

/**
 * NOTES ON A DIFF GO AS ONE MESSAGE (0.376).
 *
 * A "+" on a line of a teammate's diff pins a note there; the notes wait as
 * one tile in the chat box; the next message carries them all, each with its
 * file, line and the line itself. Orca and Vibe Kanban both batch them, since
 * one at a time "causes the agent to swing back and forth".
 */
const DIFF = [
  'diff --git a/src/app.ts b/src/app.ts',
  '--- a/src/app.ts',
  '+++ b/src/app.ts',
  '@@ -10,3 +10,3 @@',
  ' import { serve } from "./server"',
  '-const port = 3000',
  '+const port = 3001',
  ' serve(port)'
].join('\n')
const [FILE] = parseUnifiedDiff(DIFF)
const rows = FILE!.hunks[0]!.rows
const added = rows.find((row) => row.kind === 'add')!
const removed = rows.find((row) => row.kind === 'del')!

describe('a note', () => {
  it('knows its file and the line a person reads: the new one, or the old one for a removed line', () => {
    expect(diffNoteFor('src/app.ts', added, '  read it from PORT  ')).toMatchObject({ path: 'src/app.ts', line: 11, kind: 'add', code: 'const port = 3001', text: 'read it from PORT' })
    expect(diffNoteFor('src/app.ts', removed, 'why remove this?')).toMatchObject({ line: 11, kind: 'del' })
  })

  it('replaces the note already on its line', () => {
    const first = diffNoteFor('src/app.ts', added, 'one')
    const second = diffNoteFor('src/app.ts', added, 'two')
    expect(withNote([first], second)).toEqual([second])
  })
})

describe('the notes, as the message carries them', () => {
  const notes: DiffNote[] = [
    diffNoteFor('src/app.ts', added, 'Read it from the PORT variable instead.'),
    diffNoteFor('README.md', { kind: 'context', oldNo: 4, newNo: 4, text: 'Run it with npm start.' }, 'This should say pnpm.'),
    diffNoteFor('src/app.ts', removed, 'Keep a comment saying why it moved.')
  ]

  it('is one block, in file then line order, each note with the line it is about', () => {
    expect(diffNotesBlock(notes)).toBe(
      [
        'Notes on your changes (3):',
        '- README.md, line 4 `Run it with npm start.`: This should say pnpm.',
        '- src/app.ts, line 11 `const port = 3001`: Read it from the PORT variable instead.',
        '- src/app.ts, removed line 11 `const port = 3000`: Keep a comment saying why it moved.'
      ].join('\n')
    )
  })

  it('follows the person’s words, and is the whole message when there are none', () => {
    expect(withDiffNotes('Nearly there.', notes.slice(0, 1))).toBe(`Nearly there.\n\n${diffNotesBlock(notes.slice(0, 1))}`)
    expect(withDiffNotes('', notes.slice(0, 1))).toBe(diffNotesBlock(notes.slice(0, 1)))
    expect(withDiffNotes('Just this.', [])).toBe('Just this.')
  })
})

describe('a diff', () => {
  const place = (notes: readonly DiffNote[]): DiffNotesPlace => ({ notes, add: () => undefined, remove: () => undefined, who: 'Wren' })
  const view = (value: DiffNotesPlace | undefined): string =>
    renderToStaticMarkup(
      <DiffNotesContext.Provider value={value}>
        <DiffView file={FILE!} truncated={false} reported={undefined} />
      </DiffNotesContext.Provider>
    )

  it('offers a note on every line where one can be sent, and nowhere else', () => {
    expect([...view(place([])).matchAll(/lc-diff__noteadd/g)]).toHaveLength(rows.length)
    expect(view(place([]))).toContain('aria-label="Add a note on src/app.ts, line 11"')
    expect(view(undefined)).not.toContain('lc-diff__noteadd')
  })

  it('shows a note under its line, and marks the line', () => {
    const html = view(place([diffNoteFor('src/app.ts', added, 'Read it from PORT.')]))
    expect(html).toContain('lc-diff__row is-add has-note')
    expect(html).toContain('<span class="lc-diff__notetext">Read it from PORT.</span>')
    expect(diffNoteKey('src/app.ts', added)).toBe('src/app.ts:add::11')
  })
})

describe('the chat box', () => {
  const composer = (over: Partial<ComposerProps>): string =>
    renderToStaticMarkup(
      <Composer
        {...({
          runtimes: [],
          limitedRuntimes: new Map(),
          discoveryPhase: 'ready',
          running: false,
          cancelling: false,
          activeRoute: undefined,
          error: undefined,
          mode: 'accept-edits',
          onModeChange: () => undefined,
          route: { runtime: 'codex', model: 'account-default' },
          onRouteChange: () => undefined,
          models: [],
          resolvedModels: new Map(),
          recentRoutes: [],
          platform: 'win32',
          effort: undefined,
          onEffortChange: () => undefined,
          onStart: async () => true,
          onCancel: () => undefined,
          onOpenRoutePicker: () => undefined,
          onHandOff: () => undefined,
          handingOff: false,
          workspaceName: 'shop',
          workspacePath: 'C:\\work\\shop',
          onChooseFolder: () => undefined,
          teammateName: undefined,
          busyWith: undefined,
          queued: undefined,
          queuedNote: undefined,
          onQueue: () => undefined,
          onUnqueue: () => undefined,
          onSendQueued: () => undefined,
          continuationNote: undefined,
          queuedElsewhere: false,
          ...over
        } as ComposerProps)}
      />
    )

  it('holds the notes as one tile that says how many, and none when there are none', () => {
    const two = [diffNoteFor('src/app.ts', added, 'a'), diffNoteFor('src/app.ts', removed, 'b')]
    expect(composer({ diffNotes: two, onClearDiffNotes: () => undefined })).toContain(`>${diffNotesTile(2)}</span>`)
    expect(diffNotesTile(1)).toBe('1 note on the changes')
    expect(composer({ diffNotes: [], onClearDiffNotes: () => undefined })).not.toContain('notes on the changes')
  })
})
