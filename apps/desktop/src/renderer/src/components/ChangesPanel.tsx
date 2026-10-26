import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { FolderDiff } from '../../../shared/folder-diff.js'
import type { Walkthrough } from '../../../shared/walkthrough.js'
import { parseUnifiedDiff } from '../diff.js'
import type { DiffFile } from '../diff.js'
import { agoLabel } from '../teammateWork.js'
import { DiffView } from './DiffView.js'
import { Icon } from './Icon.js'
import { InlineProse } from './ThreadItems.js'
import { fileMenuEvent } from '../fileMenuOpen.js'

/**
 * THE FOLDER'S CHANGES, IN ONE PANEL (0.732, shared/folder-diff.ts), laid out as Claude Code's own (claude.ai/code,
 * 2026-10-10): `main → <branch>` with the count at the top; the files as a tree beside every file's diff in one
 * scroll, each under a header that folds it; the branch's commits under the tree, any one of which can be looked
 * at alone. Reads only.
 */
/**
 * WALK ME THROUGH IT (0.771, shared/walkthrough.ts): `walkWith` is the conversation on screen. Its teammate goes
 * through the change shown as its author would in a review -- the summary over the files, the files in the order to
 * read them, a note on each -- and the panel shows it so until it is closed, another commit is picked, or the
 * change is read again.
 */
type Walk =
  | { readonly phase: 'asking' }
  | { readonly phase: 'shown'; readonly walkthrough: Walkthrough }
  | { readonly phase: 'failed'; readonly message: string }

export function ChangesPanel({ workspacePath, running, onClose, walkWith, committed = 0 }: { readonly workspacePath?: string; readonly running: boolean; readonly onClose: () => void; readonly walkWith?: { readonly of: string; readonly teammate: string; readonly asks: readonly string[] }; /** Counts commits made from the Commit button: each one is a change to read (2026-10-16). */ readonly committed?: number }): ReactElement {
  const [diff, setDiff] = useState<FolderDiff>()
  const [showing, setShowing] = useState<'all' | string>('all')
  // The tree beside the diffs where there is room for both; in a small window, one press away.
  const [showFiles, setShowFiles] = useState(() => typeof window === 'undefined' || window.innerWidth >= 1300)
  const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set())
  const [menuOpen, setMenuOpen] = useState(false)
  const [asked, setAsked] = useState(0)
  const scroller = useRef<HTMLDivElement>(null)
  const [walk, setWalk] = useState<Walk>()
  // A walk is of the change it was asked about: another commit picked, the change read again, or a turn that may
  // change it, ends it.
  useEffect(() => {
    setWalk(undefined)
  }, [showing, asked, running])
  const walkThrough = async (): Promise<void> => {
    const bridge = window.desktop
    if (walkWith === undefined || bridge?.walkThroughChange === undefined) return
    setWalk({ phase: 'asking' })
    const answer = await bridge
      .walkThroughChange(walkWith.of, walkWith.asks, showing === 'all' ? undefined : showing)
      .catch(() => ({ ok: false as const, message: 'Locust could not ask for it. The changes are as they were.' }))
    setWalk(answer.ok ? { phase: 'shown', walkthrough: answer.walkthrough } : { phase: 'failed', message: answer.message })
    scroller.current?.scrollTo({ top: 0 })
  }

  // Read when opened, when a turn ends (it may have changed files), on Refresh, and when Commit made a commit -- the
  // panel kept showing a change already committed until the next turn (Gemini 4 Argon's workflow drive, 2026-10-16).
  useEffect(() => {
    if (running) return
    let current = true
    void window.desktop
      ?.folderDiff(showing === 'all' ? undefined : showing)
      .then((read) => {
        if (!current) return
        setDiff(read)
        // The commit picked has left the branch (an amend, a rebase): all the changes came back, so All is picked (0.753, CH4).
        if (read.kind === 'diff' && read.showing === 'all' && showing !== 'all') setShowing('all')
      })
      .catch(() => {
        if (current) setDiff({ kind: 'none', why: 'not-a-repository' })
      })
    return () => {
      current = false
    }
  }, [showing, running, asked, committed])

  const read = useMemo(() => (diff?.kind === 'diff' ? parseUnifiedDiff(diff.text) : []), [diff])
  // Walked through, each file has its note and its place in the reading; one it did not mention keeps git's place after.
  const notes = useMemo(
    () => (walk?.phase === 'shown' ? new Map(walk.walkthrough.files.map((file, index) => [file.path, { ...file, order: index + 1 }])) : undefined),
    [walk]
  )
  const files = useMemo(
    () => (notes === undefined ? read : read.map((file, index) => ({ file, rank: notes.get(file.path)?.order ?? 10_000 + index })).sort((a, b) => a.rank - b.rank).map(({ file }) => file)),
    [read, notes]
  )
  const totals = useMemo(() => files.reduce((sum, file) => ({ added: sum.added + countOf(file).added, removed: sum.removed + countOf(file).removed }), { added: 0, removed: 0 }), [files])
  const jump = (path: string): void => {
    setFolded((current) => {
      if (!current.has(path)) return current
      const next = new Set(current)
      next.delete(path)
      return next
    })
    scroller.current?.querySelector(`[data-path="${CSS.escape(path)}"]`)?.scrollIntoView({ block: 'start' })
  }

  return (
    <aside className="lc-viewer lc-changes" aria-label="Changes">
      <div className="lc-viewer__head lc-changes__head">
        <button
          type="button"
          className={`lc-viewer__action${showFiles ? ' is-on' : ''}`}
          aria-pressed={showFiles}
          aria-label="Show files"
          title="Show the files (Ctrl+Shift+Y)"
          onClick={() => setShowFiles(!showFiles)}
        >
          <Icon name="columns" size={13} />
        </button>
        <span className="lc-changes__branch lc-mono">
          {diff?.kind === 'diff' ? (
            <>
              {diff.base} <span aria-hidden="true">→</span> {diff.branch}
            </>
          ) : (
            'Changes'
          )}
        </span>
        {files.length > 0 && (
          <span className="lc-changes__count lc-mono">
            <span className="lc-diff__addmark">+{totals.added}</span> <span className="lc-diff__delmark">−{totals.removed}</span>
          </span>
        )}
        <span className="lc-viewer__spacer" />
        {walkWith !== undefined && read.length > 0 && (walk === undefined || walk.phase === 'failed') && (
          <button type="button" className="lc-changes__walk" title={`${walkWith.teammate} goes through the change as its author would in a review`} onClick={() => void walkThrough()}>
            <Icon name="message" size={12} />
            Walk me through it
          </button>
        )}
        <span className="lc-changes__menuwrap">
          <button type="button" className="lc-viewer__action" aria-label="More" aria-expanded={menuOpen} title="More" onClick={() => setMenuOpen(!menuOpen)}>
            <Icon name="dots" size={13} />
          </button>
          {menuOpen && (
            <span className="lc-changes__menu" role="menu">
              {[
                { label: showFiles ? 'Hide files' : 'Show files', run: () => setShowFiles(!showFiles) },
                { label: 'Collapse all files', run: () => setFolded(new Set(files.map((file) => file.path))) },
                { label: 'Expand all files', run: () => setFolded(new Set()) },
                { label: 'Refresh', run: () => setAsked((count) => count + 1) }
              ].map((item) => (
                <button
                  key={item.label}
                  type="button"
                  role="menuitem"
                  className="lc-changes__menuitem"
                  onClick={() => {
                    item.run()
                    setMenuOpen(false)
                  }}
                >
                  {item.label}
                </button>
              ))}
            </span>
          )}
        </span>
        <button type="button" className="lc-viewer__close" aria-label="Close" title="Close" onClick={onClose}>
          <Icon name="close" size={13} />
        </button>
      </div>
      <div
        className="lc-changes__body"
        onKeyDown={(event) => {
          if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === 'y') {
            event.preventDefault()
            setShowFiles(!showFiles)
          }
        }}
      >
        {diff === undefined ? (
          <p className="lc-changes__note">Reading the folder’s changes…</p>
        ) : diff.kind === 'none' ? (
          <p className="lc-changes__note">{diff.why === 'clean' ? 'Nothing has changed here since the branch left its base.' : 'This folder is not a git repository, so there are no changes to show.'}</p>
        ) : (
          <>
            {showFiles && (
              <nav className="lc-changes__side" aria-label="Changed files">
                <FileTree files={files} onJump={jump} />
                {diff.commits.length > 0 && (
                  <div className="lc-changes__commits">
                    <div className="lc-changes__commitshead">
                      <span>Commits</span>
                      <span className="lc-mono">{diff.commits.length}</span>
                    </div>
                    <button type="button" className={`lc-changes__commit${showing === 'all' ? ' is-active' : ''}`} onClick={() => setShowing('all')}>
                      All changes
                    </button>
                    {diff.commits.map((commit) => (
                      <button
                        key={commit.sha}
                        type="button"
                        className={`lc-changes__commit${showing === commit.sha ? ' is-active' : ''}`}
                        title={commit.subject}
                        onClick={() => setShowing(commit.sha)}
                      >
                        <span className="lc-changes__commitsubject">{commit.subject}</span>
                        <span className="lc-changes__commitmeta lc-mono">
                          {commit.short} · {commit.author} · {agoLabel(commit.at) ?? ''}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </nav>
            )}
            <div className="lc-changes__diffs" ref={scroller}>
              {walk !== undefined && (
                <div className="lc-changes__walkthrough" role="status" aria-live="polite" aria-label="Walkthrough">
                  {walk.phase === 'asking' ? (
                    <p className="lc-changes__walksaid">{`${walkWith?.teammate ?? 'Your teammate'} is walking through the change…`}</p>
                  ) : walk.phase === 'failed' ? (
                    <p className="lc-changes__walksaid">{walk.message}</p>
                  ) : (
                    <>
                      <div className="lc-changes__walkhead">
                        <span className="lc-changes__walkwho">{`${walkWith?.teammate ?? 'Your teammate'}’s walkthrough`}</span>
                        <button type="button" className="lc-viewer__action" aria-label="Close the walkthrough" title="Close the walkthrough" onClick={() => setWalk(undefined)}>
                          <Icon name="close" size={12} />
                        </button>
                      </div>
                      <p className="lc-changes__walksummary"><InlineProse text={walk.walkthrough.summary} /></p>
                    </>
                  )}
                </div>
              )}
              {files.length === 0 && <p className="lc-changes__note">This commit changes no files Locust can show.</p>}
              {files.map((file) => {
                const open = !folded.has(file.path)
                const { added, removed } = countOf(file)
                const at = file.path.lastIndexOf('/')
                const note = notes?.get(file.path)
                return (
                  <section className="lc-changes__file" key={file.path} data-path={file.path}>
                    <button
                      type="button"
                      className="lc-changes__filehead"
                      aria-expanded={open}
                      onContextMenu={fileMenuEvent(file.path)}
                      onClick={() =>
                        setFolded((current) => {
                          const next = new Set(current)
                          if (next.has(file.path)) next.delete(file.path)
                          else next.add(file.path)
                          return next
                        })
                      }
                    >
                      <Icon name={open ? 'chevron-down' : 'chevron-right'} size={12} />
                      <span className="lc-changes__filename">{at < 0 ? file.path : file.path.slice(at + 1)}</span>
                      {at > 0 && <span className="lc-changes__filedir lc-mono">{file.path.slice(0, at)}</span>}
                      <span className="lc-changes__filecount lc-mono">
                        <span className="lc-diff__addmark">+{added}</span> <span className="lc-diff__delmark">−{removed}</span>
                      </span>
                    </button>
                    {note !== undefined && (
                      <div className="lc-changes__why">
                        <span className="lc-changes__order lc-mono" aria-label={`Read it ${String(note.order)} of ${String(notes?.size ?? 0)}`}>{note.order}</span>
                        <div className="lc-changes__whytext">
                          <p><InlineProse text={note.why} /></p>
                          {note.check !== undefined && (
                            <p className="lc-changes__check">
                              <span className="lc-changes__checklabel">Check</span>
                              <InlineProse text={note.check} />
                            </p>
                          )}
                        </div>
                      </div>
                    )}
                    {open && <DiffView file={file} truncated={false} reported={undefined} workspacePath={workspacePath} />}
                  </section>
                )
              })}
              {diff.truncated && <p className="lc-changes__note">The rest is too large to show here. Your own git has all of it.</p>}
            </div>
          </>
        )}
      </div>
    </aside>
  )
}

/** A file's added and removed lines, from its rows. */
export function countOf(file: DiffFile): { readonly added: number; readonly removed: number } {
  let added = 0
  let removed = 0
  for (const hunk of file.hunks) {
    for (const row of hunk.rows) {
      if (row.kind === 'add') added += 1
      else if (row.kind === 'del') removed += 1
    }
  }
  return { added, removed }
}

interface TreeFolder {
  readonly name: string
  readonly folders: Map<string, TreeFolder>
  readonly files: DiffFile[]
}

/** The changed files as folders, each folder's own files under it, folders first (Claude's "Group files by folder"). */
export function treeOf(files: readonly DiffFile[]): TreeFolder {
  const root: TreeFolder = { name: '', folders: new Map(), files: [] }
  for (const file of files) {
    const parts = file.path.split('/')
    let at = root
    for (const part of parts.slice(0, -1)) {
      let next = at.folders.get(part)
      if (next === undefined) {
        next = { name: part, folders: new Map(), files: [] }
        at.folders.set(part, next)
      }
      at = next
    }
    at.files.push(file)
  }
  return root
}

function FileTree({ files, onJump }: { readonly files: readonly DiffFile[]; readonly onJump: (path: string) => void }): ReactElement {
  const [closed, setClosed] = useState<ReadonlySet<string>>(() => new Set())
  const draw = (folder: TreeFolder, prefix: string, depth: number): ReactElement[] => [
    ...[...folder.folders.values()].flatMap((child) => {
      const key = `${prefix}${child.name}/`
      const open = !closed.has(key)
      return [
        <button
          key={key}
          type="button"
          className="lc-changes__treefolder"
          style={{ paddingLeft: `${String(8 + depth * 12)}px` }}
          aria-expanded={open}
          onClick={() =>
            setClosed((current) => {
              const next = new Set(current)
              if (next.has(key)) next.delete(key)
              else next.add(key)
              return next
            })
          }
        >
          <Icon name={open ? 'chevron-down' : 'chevron-right'} size={11} />
          <span>{child.name}</span>
        </button>,
        ...(open ? draw(child, key, depth + 1) : [])
      ]
    }),
    ...folder.files.map((file) => {
      const { added, removed } = countOf(file)
      return (
        <button
          key={file.path}
          type="button"
          className="lc-changes__treefile"
          style={{ paddingLeft: `${String(20 + depth * 12)}px` }}
          onContextMenu={fileMenuEvent(file.path)}
          title={file.path}
          onClick={() => onJump(file.path)}
        >
          <span className="lc-changes__treename">{file.path.split('/').pop()}</span>
          <span className="lc-changes__treecount lc-mono">
            <span className="lc-diff__addmark">+{added}</span> <span className="lc-diff__delmark">−{removed}</span>
          </span>
        </button>
      )
    })
  ]
  return <div className="lc-changes__tree">{draw(treeOf(files), '', 0)}</div>
}
