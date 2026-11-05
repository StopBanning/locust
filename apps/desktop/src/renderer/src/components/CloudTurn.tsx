import { useEffect, useId, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import { cloudChangeWords, CLOUD_MACHINE_WORDS, type PublicCloudTurn } from '../../../shared/cloud-computer.js'
import { commitDraft } from '../../../shared/folder-commit.js'
import { fileCounts, parseUnifiedDiff } from '../diff.js'
import { agoLabel } from '../teammateWork.js'
import { DiffView } from './DiffView.js'
import { GitHubMark } from './GitHubAccount.js'
import { Icon } from './Icon.js'
import { LineCounts } from './LineCounts.js'

/**
 * A TURN ON YOUR CLOUD COMPUTER, IN THE THREAD (0.761). The turn's work streams in like any turn's; these two say
 * what is different about it: where it ran -- a line between the person's words and the work -- and, once it has
 * ended, what became of its change, which stays on the cloud computer until the person brings it home or sends it
 * to a pull request (0.762).
 */
export function CloudMarker({ turn }: { readonly turn: PublicCloudTurn }): ReactElement {
  return (
    <div className="lc-cloudmarker" role="note">
      <span className="lc-cloudmarker__rule" aria-hidden="true" />
      <Icon name="cloud" size={14} />
      <span className="lc-cloudmarker__words">On your cloud computer</span>
      <span className="lc-cloudmarker__where lc-mono">{`${turn.repo.split('/').pop() ?? turn.repo}${turn.branch === undefined ? '' : ` · ${turn.branch}`} · ${CLOUD_MACHINE_WORDS}`}</span>
      <span className="lc-cloudmarker__rule" aria-hidden="true" />
    </div>
  )
}

/** How many files the card lists before it says how many more. */
const FILES_LISTED = 6

export function CloudChangeCard({
  turn,
  onPatch,
  onApply,
  onDiscard,
  onPullRequest,
  asks = [],
  teammate
}: {
  readonly turn: PublicCloudTurn
  readonly onPatch: (missionId: string) => Promise<string | undefined>
  readonly onApply: (missionId: string) => Promise<string | undefined>
  readonly onDiscard: (missionId: string) => Promise<string | undefined>
  /** Sends the change to a pull request on GitHub (0.762): what went wrong, or undefined once it is there. */
  readonly onPullRequest?: (missionId: string, message: string) => Promise<string | undefined>
  /** What the person asked for the turns this change holds, oldest first: the pull request's message is drafted from it. */
  readonly asks?: readonly string[]
  /** The teammate whose change it is, named in the draft. */
  readonly teammate?: string
}): ReactElement | null {
  const [shown, setShown] = useState<{ readonly loading: boolean; readonly patch: string | undefined }>()
  const [busy, setBusy] = useState<'apply' | 'discard' | 'pull-request'>()
  const [problem, setProblem] = useState<string>()
  const [confirming, setConfirming] = useState(false)
  /** The pull request's message while it is being written (0.762); undefined when the card shows its buttons. */
  const [composing, setComposing] = useState<string>()
  /** The person's message, kept when a send is refused: the card goes back to its buttons, and the next press brings it back. */
  const [kept, setKept] = useState<string>()
  const [refusal, setRefusal] = useState<string>()
  const messageBox = useRef<HTMLTextAreaElement>(null)
  const messageId = useId()
  const files = turn.files ?? []
  const writing = composing !== undefined

  // The box takes the keys when it opens, so the person can read the draft and send it from where they are.
  useEffect(() => {
    if (writing) messageBox.current?.focus()
  }, [writing])

  if (turn.state === 'carried') return null
  // The turn has ended (the thread draws this only then), and what it changed there is being read: a few seconds.
  if (turn.state === 'working') return <div className="lc-thread__note" role="status">Reading what it changed on your cloud computer…</div>
  if (turn.state === 'none') return <div className="lc-thread__note">No files changed on your cloud computer</div>
  if (turn.state === 'discarded') return <div className="lc-thread__note">Its change was put back on your cloud computer</div>
  if (turn.state === 'failed') {
    return <div className="lc-thread__note">{`What it changed on your cloud computer could not be read. ${turn.problem ?? ''}`.trim()}</div>
  }
  if (turn.state === 'home') {
    const when = turn.appliedAt === undefined ? undefined : agoLabel(turn.appliedAt)
    return (
      <div className="lc-cloudchange is-home" role="status">
        <Icon name="check" size={13} />
        <span>{`Brought into this folder${when === undefined ? '' : ` ${when}`}`}</span>
        {files.length > 0 && <span className="lc-cloudchange__counts lc-mono">{cloudChangeWords(files)}</span>}
      </div>
    )
  }
  if (turn.state === 'pr') {
    const request = turn.pullRequest
    return (
      <div className="lc-cloudchange is-home is-pr" role="status">
        <GitHubMark size={13} />
        <span>{request === undefined ? 'Sent to a pull request on GitHub' : `Sent to pull request #${String(request.number)}`}</span>
        {request?.into !== undefined && <span className="lc-cloudchange__counts lc-mono">{`into ${request.into}`}</span>}
        {files.length > 0 && <span className="lc-cloudchange__counts lc-mono">{cloudChangeWords(files)}</span>}
        {request !== undefined && (
          // A button that asks the host, which may refuse an address and says why -- never an anchor (no-dead-links).
          <button
            type="button"
            className="lc-commit__link"
            onClick={() => {
              void window.desktop
                ?.openLink(request.url)
                .then((answer) => setRefusal(answer.ok ? undefined : answer.message))
                .catch(() => setRefusal('That link could not be opened. Nothing on this machine changed.'))
            }}
          >
            Open on GitHub
          </button>
        )}
        {refusal !== undefined && <span className="lc-cloudchange__problem">{refusal}</span>}
      </div>
    )
  }

  const show = (): void => {
    if (shown !== undefined) {
      setShown(undefined)
      return
    }
    setShown({ loading: true, patch: undefined })
    void onPatch(turn.missionId).then((patch) => setShown({ loading: false, patch }))
  }
  const act = (which: 'apply' | 'discard'): void => {
    setBusy(which)
    setProblem(undefined)
    void (which === 'apply' ? onApply(turn.missionId) : onDiscard(turn.missionId))
      .then((said) => {
        if (said !== undefined) setProblem(said)
      })
      .finally(() => {
        setBusy(undefined)
        setConfirming(false)
      })
  }
  const send = (): void => {
    if (onPullRequest === undefined || composing === undefined || composing.trim().length === 0) return
    setBusy('pull-request')
    setProblem(undefined)
    const words = composing
    void onPullRequest(turn.missionId, words)
      .then((said) => {
        // Done, the card becomes the line that names the pull request. Refused, it goes back to its buttons with the
        // reason -- usually "Apply it here instead", which a form with the same button again would not let it say --
        // and the words wait for the next press.
        setComposing(undefined)
        setKept(said === undefined ? undefined : words)
        if (said !== undefined) setProblem(said)
      })
      .finally(() => setBusy(undefined))
  }
  const added = files.reduce((sum, file) => sum + file.added, 0)
  const removed = files.reduce((sum, file) => sum + file.removed, 0)
  const parsed = shown?.patch === undefined ? [] : parseUnifiedDiff(shown.patch)
  const open = turn.openPullRequest

  return (
    <section className="lc-cloudchange" aria-label="The change on your cloud computer">
      <div className="lc-cloudchange__head">
        <Icon name="cloud" size={15} />
        <h3 className="lc-cloudchange__title">The change is on your cloud computer</h3>
        <span className="lc-cloudchange__counts lc-mono">
          {`${String(files.length)} ${files.length === 1 ? 'file' : 'files'} · `}
          {/* Counts that are both zero draw nothing, and their dot goes with them. */}
          {added + removed > 0 && (
            <>
              <LineCounts added={added} removed={removed} />
              {' · '}
            </>
          )}
          {'not here yet'}
        </span>
      </div>
      <ul className="lc-cloudchange__files lc-mono">
        {files.slice(0, FILES_LISTED).map((file) => (
          <li key={file.path}>
            <span className="lc-cloudchange__path">{file.path}</span>
            {file.binary === true ? <span className="lc-cloudchange__binary">binary</span> : (
              <>
                <LineCounts added={file.added} removed={file.removed} />
              </>
            )}
          </li>
        ))}
        {files.length > FILES_LISTED && <li className="lc-cloudchange__more">{`and ${String(files.length - FILES_LISTED)} more`}</li>}
      </ul>
      {problem !== undefined && <p className="lc-cloudchange__problem" role="alert">{problem}</p>}
      {writing ? (
        /*
         * A PULL REQUEST FROM THE CLOUD COMPUTER (0.762), the canvas's third button. The message is drafted as
         * Commit drafts one -- from what was asked, the teammate named -- and its first line is the title. Only the
         * change goes: a branch of its own from GitHub's copy, committed as the person, pushed by Locust there.
         */
        <div className="lc-cloudchange__pr" role="group" aria-label={open === undefined ? 'Open a pull request' : `Add to pull request #${String(open.number)}`}>
          <label className="lc-commit__label" htmlFor={messageId}>Message</label>
          <textarea
            id={messageId}
            ref={messageBox}
            className="lc-commit__message"
            rows={4}
            value={composing}
            spellCheck
            disabled={busy !== undefined}
            onChange={(event) => setComposing(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape' && busy === undefined) setComposing(undefined)
            }}
          />
          <div className="lc-cloudchange__actions">
            <button type="button" className="lc-primarybutton" disabled={busy !== undefined || composing.trim().length === 0} onClick={send}>
              {busy === 'pull-request'
                ? open === undefined ? 'Opening…' : 'Adding…'
                : open === undefined ? 'Open the pull request' : `Add to pull request #${String(open.number)}`}
            </button>
            <button type="button" className="lc-button" disabled={busy !== undefined} onClick={() => setComposing(undefined)}>Cancel</button>
          </div>
        </div>
      ) : confirming ? (
        <div className="lc-cloudchange__actions" role="group" aria-label="Discard the change">
          <span className="lc-cloudchange__ask">Put the cloud copy back to where this conversation began there?</span>
          <button type="button" className="lc-button lc-button--danger" disabled={busy !== undefined} onClick={() => act('discard')}>
            {busy === 'discard' ? 'Discarding…' : 'Discard'}
          </button>
          <button type="button" className="lc-button" disabled={busy !== undefined} onClick={() => setConfirming(false)}>Keep it</button>
        </div>
      ) : (
        <div className="lc-cloudchange__actions">
          <button type="button" className="lc-button" aria-expanded={shown !== undefined} onClick={show}>
            <Icon name="diff" size={13} /> {shown === undefined ? 'Show the change' : 'Hide the change'}
          </button>
          <button
            type="button"
            className="lc-primarybutton"
            disabled={busy !== undefined}
            title="Its change comes into this folder, not committed, so you can look before you keep it"
            onClick={() => act('apply')}
          >
            {busy === 'apply' ? 'Applying…' : 'Apply to this folder'}
          </button>
          {onPullRequest !== undefined && (
            <button
              type="button"
              className="lc-button"
              disabled={busy !== undefined}
              onClick={() => {
                setProblem(undefined)
                setComposing(kept ?? commitDraft({ asks, ...(teammate === undefined ? {} : { teammate }) }))
              }}
            >
              <GitHubMark size={13} /> {open === undefined ? 'Open a pull request' : `Add to pull request #${String(open.number)}`}
            </button>
          )}
          <button type="button" className="lc-button" disabled={busy !== undefined} onClick={() => setConfirming(true)}>Discard…</button>
        </div>
      )}
      <p className="lc-cloudchange__note">
        {writing
          ? open === undefined
            ? 'The first line is its title. It goes to GitHub on a branch of its own, holding only this change and committed as you: nothing else in this folder goes with it.'
            : `It goes to pull request #${String(open.number)} as one more commit, made as you: nothing else in this folder goes with it.`
          : 'Apply brings it into this folder, not committed, so you can look before you keep it. Or reply below: the next turn carries on from it there.'}
      </p>
      {shown !== undefined && (
        <div className="lc-review__files">
          {shown.loading && <p className="lc-cloudchange__note">Reading the change…</p>}
          {!shown.loading && shown.patch === undefined && <p className="lc-cloudchange__note">Your cloud computer did not answer. Try again in a moment.</p>}
          {parsed.map((file) => {
            const counts = fileCounts(file)
            return (
              <section key={`${turn.missionId}:${file.path}`} aria-label={file.path}>
                <div className="lc-review__path lc-mono">
                  <span className="lc-review__name">{file.path}</span>
                  {file.status !== 'MODIFIED' && <span className="lc-review__status">{file.status.toLowerCase()}</span>}
                  <LineCounts added={counts.added} removed={counts.removed} />
                </div>
                <DiffView file={file} truncated={false} reported={undefined} />
              </section>
            )
          })}
        </div>
      )}
    </section>
  )
}
