import { useCallback, useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import { blockedSentence, commitDraft } from '../../../shared/folder-commit.js'
import type { CommitResult, CommitThen, FolderChanges } from '../../../shared/folder-commit.js'
import { Icon } from './Icon.js'

/**
 * COMMIT (0.680): the folder's changes, committed from the conversation that made them.
 *
 * The Codex app and Claude Code each have a button for this; Locust sent the person to a terminal. In the
 * conversation's header, only while the folder has something uncommitted and nothing is running: the files
 * a commit would take, a message drafted from what was asked, and Commit -- or Commit and push, or Commit
 * and open a pull request, when the folder can. main/folder-commit.ts does the git, as the person.
 *
 * WRITE IT FROM THE CHANGES (0.771, shared/commit-draft.ts): the teammate that made them writes the message from
 * the diff instead -- the subject a pull request takes as its title, the body as its description -- into the box,
 * to be read and changed before Commit.
 */
const SHOWN_FILES = 6
const STATUS_LETTER = { added: 'A', modified: 'M', deleted: 'D', renamed: 'R' } as const

export function CommitChanges({
  running,
  asks,
  teammate,
  of,
  fixes,
  onCommitted
}: {
  /** A run is live in this conversation: nothing is offered until it ends, so half a turn is never committed. */
  readonly running: boolean
  /** What the person asked in this conversation, oldest first: the draft message is made from it. */
  readonly asks: readonly string[]
  /** The teammate whose conversation this is, named in the draft. */
  readonly teammate?: string
  /** The conversation's latest turn: its teammate writes the message from the changes (0.771). */
  readonly of?: string
  /** The issue this conversation started from, named once in the pull request. */
  readonly fixes?: number
  /** A commit was made: what shows the folder's changes reads them again. */
  readonly onCommitted?: () => void
}): ReactElement | null {
  const [changes, setChanges] = useState<FolderChanges>()
  const [open, setOpen] = useState(false)
  const [message, setMessage] = useState('')
  const [working, setWorking] = useState<CommitThen>()
  const [result, setResult] = useState<CommitResult>()
  /** Every file listed, not the first six (Sol's review: the rest could not be seen before committing). */
  const [allShown, setAllShown] = useState(false)
  const [drafting, setDrafting] = useState(false)
  /** Why the teammate's message is not in the box, when it is not. */
  const [draftSaid, setDraftSaid] = useState<string>()
  const panel = useRef<HTMLDivElement>(null)

  const draft = async (): Promise<void> => {
    const bridge = window.desktop
    if (of === undefined || bridge?.draftCommitMessage === undefined) return
    setDrafting(true)
    setDraftSaid(undefined)
    const answer = await bridge.draftCommitMessage(of, asks).catch(() => ({ ok: false as const, message: 'Locust could not ask for it. The message you had is still in the box.' }))
    setDrafting(false)
    // Who made the changes stays on the commit, as the drafted message says it (commitDraft's trailer).
    if (answer.ok) setMessage(teammate === undefined ? answer.message : `${answer.message}\n\nLocust-Teammate: ${teammate}`)
    else setDraftSaid(answer.message)
  }
  const read = useCallback(async (): Promise<void> => {
    const answer = await window.desktop?.folderChanges().catch(() => undefined)
    setChanges(answer)
  }, [])
  // Read when the conversation is shown, when a run ends, and when the window comes back: never on a timer.
  useEffect(() => {
    if (running) return
    void read()
    const again = (): void => void read()
    window.addEventListener('focus', again)
    return () => window.removeEventListener('focus', again)
  }, [running, read])

  useEffect(() => {
    if (!open) return
    const away = (event: MouseEvent): void => {
      if (working !== undefined || drafting) return
      const target = event.target as Node | null
      if (target !== null && panel.current?.parentElement?.contains(target) === true) return
      setOpen(false)
    }
    const escape = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && working === undefined) setOpen(false)
    }
    document.addEventListener('mousedown', away)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('mousedown', away)
      document.removeEventListener('keydown', escape)
    }
  }, [open, working])

  const shown = open || (changes !== undefined && changes.kind !== 'none' && !running)
  if (!shown) return null

  const go = async (then: CommitThen): Promise<void> => {
    setWorking(then)
    // The files the person was shown: the host commits those, or refuses if the folder has changed since.
    const shown = changes?.kind === 'changes' ? changes.files.map((file) => file.path) : []
    const done = await window.desktop?.commitFolder(message, then, shown, fixes).catch((): CommitResult => ({ kind: 'refused', message: 'Locust could not reach git.' }))
    setWorking(undefined)
    setResult(done)
    void read()
    if (done?.kind === 'done') onCommitted?.()
  }

  const count = changes?.kind === 'changes' ? changes.files.length : 0
  return (
    <div className="lc-commitwrap">
      <button
        type="button"
        className={`lc-button${open ? ' is-active' : ''}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label="Commit"
        title={count > 0 ? `${String(count)} ${count === 1 ? 'file' : 'files'} changed in this folder and not committed` : 'Commit this folder\'s changes'}
        onClick={() => {
          if (open) {
            if (working === undefined) setOpen(false)
            return
          }
          setResult(undefined)
          setAllShown(false)
          setDraftSaid(undefined)
          setMessage(commitDraft({ asks, ...(teammate === undefined ? {} : { teammate }) }))
          setOpen(true)
          void read()
        }}
      >
        <Icon name="commit" size={13} /> <span className="lc-commitwrap__word">Commit</span>
      </button>
      {open && (
        <div className="lc-commit" role="dialog" aria-label="Commit" ref={panel}>
          {result !== undefined ? (
            <CommitOutcome result={result} onClose={() => setOpen(false)} onBack={() => setResult(undefined)} />
          ) : changes === undefined || changes.kind === 'none' ? (
            <p className="lc-commit__said">{changes?.why === 'clean' ? 'Nothing in this folder is waiting to be committed.' : 'This folder is not a git repository.'}</p>
          ) : changes.kind === 'blocked' ? (
            <p className="lc-commit__said">{blockedSentence(changes.why)}</p>
          ) : (
            <>
              <div className="lc-commit__head">
                <span className="lc-commit__title">
                  Commit {String(changes.files.length)} {changes.files.length === 1 ? 'file' : 'files'}
                </span>
                <span className="lc-commit__branch">on {changes.branch}</span>
              </div>
              <ul className="lc-commit__files">
                {(allShown ? changes.files : changes.files.slice(0, SHOWN_FILES)).map((file) => (
                  <li key={file.path} className="lc-commit__file">
                    <span className={`lc-commit__status lc-commit__status--${file.status}`} title={file.status}>
                      {STATUS_LETTER[file.status]}
                    </span>
                    <span className="lc-commit__path" title={file.path}>{file.path}</span>
                  </li>
                ))}
                {!allShown && changes.files.length > SHOWN_FILES && (
                  <li className="lc-commit__more">
                    <button type="button" className="lc-commit__link" onClick={() => setAllShown(true)}>
                      and {String(changes.files.length - SHOWN_FILES)} more
                    </button>
                  </li>
                )}
              </ul>
              <div className="lc-commit__labelrow">
                <label className="lc-commit__label" htmlFor="lc-commit-message">Message</label>
                {of !== undefined && (
                  <button type="button" className="lc-commit__link" disabled={drafting || working !== undefined} onClick={() => void draft()}>
                    {drafting ? `${teammate ?? 'Your teammate'} is writing it…` : 'Write it from the changes'}
                  </button>
                )}
              </div>
              <textarea
                id="lc-commit-message"
                className="lc-commit__message"
                rows={4}
                value={message}
                spellCheck
                disabled={working !== undefined || drafting}
                onChange={(event) => setMessage(event.target.value)}
              />
              {draftSaid !== undefined && <p className="lc-commit__said lc-commit__said--refused">{draftSaid}</p>}
              <div className="lc-commit__actions">
                <button type="button" className="lc-primarybutton" disabled={working !== undefined || drafting || message.trim().length === 0} onClick={() => void go('commit')}>
                  {working === 'commit' ? 'Committing…' : 'Commit'}
                </button>
                {changes.remote !== undefined && (
                  <button type="button" className="lc-button" disabled={working !== undefined || drafting || message.trim().length === 0} onClick={() => void go('push')}>
                    {working === 'push' ? 'Pushing…' : 'Commit and push'}
                  </button>
                )}
                {changes.remote?.pullRequests === true && (
                  <button type="button" className="lc-button" disabled={working !== undefined || drafting || message.trim().length === 0} onClick={() => void go('pull-request')}>
                    {working === 'pull-request' ? 'Opening…' : 'Open a pull request'}
                  </button>
                )}
              </div>
              <p className="lc-commit__note">
                Made as you, with your own git name and hooks.
                {changes.remote?.pullRequests === true && changes.branch === changes.remote.defaultBranch
                  ? ` A pull request goes from a new branch; ${changes.branch} stays where it is.`
                  : ''}
              </p>
            </>
          )}
        </div>
      )}
    </div>
  )
}

function CommitOutcome({ result, onClose, onBack }: { readonly result: CommitResult; readonly onClose: () => void; readonly onBack: () => void }): ReactElement {
  // A button that asks the host, which may refuse an address and says why -- never an anchor (no-dead-links).
  const [refusal, setRefusal] = useState<string>()
  if (result.kind === 'refused') {
    return (
      <>
        <p className="lc-commit__said lc-commit__said--refused">{result.message}</p>
        <div className="lc-commit__actions">
          <button type="button" className="lc-button" onClick={onBack}>Back</button>
        </div>
      </>
    )
  }
  const files = `${String(result.files)} ${result.files === 1 ? 'file' : 'files'}`
  return (
    <>
      <p className="lc-commit__said">
        Committed {files} to {result.branch} as <code>{result.sha.slice(0, 7)}</code>.
        {result.newBranch !== undefined ? ` Your folder is on ${result.newBranch} now.` : ''}
        {result.pushed === true ? ' Pushed to origin.' : ''}
      </p>
      {result.pullRequestUrl !== undefined && (
        <p className="lc-commit__said">
          Pull request opened:{' '}
          <button
            type="button"
            className="lc-commit__link"
            onClick={() => {
              void window.desktop
                ?.openLink(result.pullRequestUrl!)
                .then((answer) => setRefusal(answer.ok ? undefined : answer.message))
                .catch(() => setRefusal('That link could not be opened. Nothing on this machine changed.'))
            }}
          >
            {result.pullRequestUrl.replace(/^https:\/\/github\.com\//, '')}
          </button>
        </p>
      )}
      {refusal !== undefined && <p className="lc-commit__said lc-commit__said--refused">{refusal}</p>}
      {result.leftOut.length > 0 && (
        <p className="lc-commit__said">Left out, too big to commit: {result.leftOut.join(', ')}.</p>
      )}
      {result.after !== undefined && <p className="lc-commit__said lc-commit__said--refused">{result.after}</p>}
      <div className="lc-commit__actions">
        <button type="button" className="lc-button" onClick={onClose}>Done</button>
      </div>
    </>
  )
}
