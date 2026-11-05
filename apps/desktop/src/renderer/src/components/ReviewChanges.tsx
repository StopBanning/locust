import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { PublicBranchReview, PublicLandBlock, PublicLandPreview } from '../../../shared/ipc.js'
import { fileCounts, parseUnifiedDiff } from '../diff.js'
import { agoLabel } from '../teammateWork.js'
import { DiffView } from './DiffView.js'
import { Icon } from './Icon.js'
import { LineCounts } from './LineCounts.js'

/**
 * REVIEW CHANGES (0.439): a teammate's own branch, seen as a whole.
 *
 * Colin picked idea #2 of PRODUCT-SUGGESTIONS-2026-09-28: each turn on Own
 * branch is committed to `locust/<name>` (turn-checkpoint.ts), and this is
 * the first of the three buttons on top -- the branch against where it left
 * the person's branch, whole or turn by turn, in the diff view the thread
 * already has. It reads; Land it (0.440, `LandCard` below) is the one
 * thing here that writes, and only when pressed.
 */
export function ReviewChanges({
  teammate,
  running,
  canEdit,
  onAsk,
  onClose
}: {
  readonly teammate: { readonly teammateId: string; readonly name: string }
  /** A run of this teammate's is live on screen: read again when it ends, since its turn is saved then. */
  readonly running: boolean
  /** The conversation's mode can edit files -- a conflict can only be resolved in one that can. */
  readonly canEdit: boolean
  /** Send a message in this teammate's conversation, as the person would. */
  readonly onAsk: (prompt: string) => Promise<boolean | string>
  readonly onClose: () => void
}): ReactElement {
  const [review, setReview] = useState<PublicBranchReview>()
  const [problem, setProblem] = useState<string>()
  /** The whole change, or one turn by its sha. */
  const [showing, setShowing] = useState<string>('whole')
  const [turnText, setTurnText] = useState<{ readonly sha: string; readonly diff: string }>()
  const [refreshed, setRefreshed] = useState(0)
  /** What the last landing, or the last ask to resolve, did -- kept while the card is gone. */
  const [landNote, setLandNote] = useState<string>()
  /*
   * A turn that ends is saved on the branch, and the branch is read again
   * without a press -- but only once the host has let the teammate go. The
   * turn ends on screen BEFORE its checkpoint is made, and the first landing
   * drive read the branch in that gap: the whole change still showed the
   * diff from before the merge the turn had just finished. The host counts
   * the teammate busy until the checkpoint is done, so that is what is waited
   * for.
   */
  const mounted = useRef(false)
  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true
      return
    }
    if (running) return
    let current = true
    void (async () => {
      const bridge = window.desktop
      for (let attempt = 0; attempt < 30 && current && bridge !== undefined; attempt += 1) {
        const answer = await bridge.landPreview(teammate.teammateId).catch(() => undefined)
        if (answer?.ok !== true || answer.data.block?.kind !== 'busy') break
        await new Promise((resolve) => setTimeout(resolve, 500))
      }
      if (current) setRefreshed((count) => count + 1)
    })()
    return () => {
      current = false
    }
  }, [running, teammate.teammateId])

  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined) return
    let current = true
    setProblem(undefined)
    void bridge.reviewBranch(teammate.teammateId).then((answer) => {
      if (!current) return
      if (answer.ok) setReview(answer.data)
      else setProblem(answer.error.message)
    }).catch(() => current && setProblem('The branch could not be read. Nothing on it was changed.'))
    return () => {
      current = false
    }
  }, [teammate.teammateId, refreshed])

  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined || showing === 'whole' || turnText?.sha === showing) return
    let current = true
    void bridge.turnDiff(teammate.teammateId, showing).then((answer) => {
      if (!current) return
      if (answer.ok) setTurnText({ sha: showing, diff: answer.data.diff })
      else setProblem(answer.error.message)
    }).catch(() => current && setProblem('That turn could not be read. The branch is as it was.'))
    return () => {
      current = false
    }
  }, [teammate.teammateId, showing, turnText?.sha])

  const text = showing === 'whole' ? review?.diff : turnText?.sha === showing ? turnText.diff : undefined
  const files = text === undefined ? [] : parseUnifiedDiff(text)
  const turns = review?.turns ?? []
  /** The whole change's files, from its diff when it was shown whole. */
  const wholeFiles = review?.diff !== undefined ? parseUnifiedDiff(review.diff).length : new Set(turns.flatMap((turn) => turn.files)).size

  return (
    <aside className="lc-viewer" aria-label="Review changes">
      <div className="lc-viewer__head">
        <Icon name="diff" size={14} />
        <span className="lc-beside__who">
          <span className="lc-viewer__name">Review {teammate.name}'s changes</span>
          <span className="lc-beside__title lc-mono">
            {review === undefined ? 'Reading the branch…' : `${review.branch}${review.against === undefined ? '' : ` against ${review.against}`}`}
          </span>
        </span>
        <span className="lc-viewer__spacer" />
        <button type="button" className="lc-viewer__action" aria-label="Read it again" title="Read the branch again" onClick={() => setRefreshed((count) => count + 1)}>
          <Icon name="refresh" size={13} />
        </button>
        <button type="button" className="lc-viewer__close" aria-label="Close" title="Close" onClick={onClose}>
          <Icon name="close" size={13} />
        </button>
      </div>
      <div className="lc-review__body">
        {problem !== undefined && <p className="lc-review__note is-warn" role="status">{problem}</p>}
        {review !== undefined && (
          <>
            {/* The news first: what the last press did. */}
            {landNote !== undefined && <p className="lc-review__note is-done" role="status">{landNote}</p>}
            <p className="lc-review__note">
              {turns.length === 0
                ? `Nothing new on ${review.branch}. Each turn ${teammate.name} finishes is saved there as it ends.`
                : `${String(turns.length)} ${turns.length === 1 ? 'turn' : 'turns'} saved on ${review.branch} since it left ${review.against ?? 'your checkout'}. Nothing here is in your branch until you land it.`}
            </p>
            {turns.length > 0 && (
              <LandCard
                key={`${review.branch}:${String(refreshed)}`}
                teammate={teammate}
                canEdit={canEdit}
                onAsk={onAsk}
                onChanged={(said) => {
                  setLandNote(said)
                  setShowing('whole')
                  setTurnText(undefined)
                  setRefreshed((count) => count + 1)
                }}
              />
            )}
            {review.uncommitted.length > 0 && (
              <p className="lc-review__note is-warn" role="status">
                Not saved on the branch yet ({String(review.uncommitted.length)}): {review.uncommitted.slice(0, 5).join(', ')}
                {review.uncommitted.length > 5 ? ` and ${String(review.uncommitted.length - 5)} more` : ''}. A turn may still be running, or these were edited by hand.
              </p>
            )}
            {turns.length > 0 && (
              <div className="lc-review__turns" role="tablist" aria-label="What to show">
                <button
                  type="button"
                  role="tab"
                  aria-selected={showing === 'whole'}
                  className={`lc-review__turn${showing === 'whole' ? ' is-active' : ''}`}
                  onClick={() => setShowing('whole')}
                >
                  <span className="lc-review__subject">The whole change</span>
                  <span className="lc-review__meta lc-mono">{fileCount(wholeFiles)}</span>
                </button>
                {turns.map((turn, index) => (
                  <button
                    key={turn.sha}
                    type="button"
                    role="tab"
                    aria-selected={showing === turn.sha}
                    className={`lc-review__turn${showing === turn.sha ? ' is-active' : ''}`}
                    title={`${turn.sha.slice(0, 12)}: ${turn.files.join(', ')}`}
                    onClick={() => setShowing(turn.sha)}
                  >
                    <span className="lc-review__subject">
                      <span className="lc-review__index lc-mono">{String(index + 1)}</span> {turn.subject}
                    </span>
                    <span className="lc-review__meta lc-mono">
                      {turn.merge === true ? `merge with ${review.against ?? 'your branch'}` : fileCount(turn.files.length)}
                      {agoLabel(turn.at) === undefined ? '' : ` · ${agoLabel(turn.at) ?? ''}`}
                    </span>
                  </button>
                ))}
              </div>
            )}
            {showing === 'whole' && review.diff === undefined && turns.length > 0 && (
              <p className="lc-review__note">The whole change is too large to show at once. Pick a turn to see it turn by turn.</p>
            )}
            {text !== undefined && text.trim().length === 0 && turns.length > 0 && (
              <p className="lc-review__note">No difference in text: the change is only to file modes or binary files.</p>
            )}
            <div className="lc-review__files">
              {files.map((file) => {
                const counts = fileCounts(file)
                return (
                  <section key={`${showing}:${file.path}`} aria-label={file.path}>
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
          </>
        )}
      </div>
    </aside>
  )
}

const fileCount = (count: number): string => `${String(count)} ${count === 1 ? 'file' : 'files'}`

const listed = (files: readonly string[]): string =>
  files.length <= 3 ? files.join(', ') : `${files.slice(0, 2).join(', ')} and ${String(files.length - 2)} more`

/** Why it cannot land now, said with what would change it. */
export function landBlockSentence(block: PublicLandBlock, name: string, onto: string | undefined): string {
  const target = onto ?? 'your branch'
  switch (block.kind) {
    case 'nothing':
      return `Nothing on ${name}'s branch is new to ${target}.`
    case 'busy':
      return `${name} is working. Land it when the turn ends.`
    case 'detached':
      return 'Your checkout is not on a branch, so there is nowhere to land it. Check out a branch, then read it again.'
    case 'old-git':
      return `Landing needs git 2.38 or newer, to check for conflicts without touching your files; this machine has ${block.version ?? 'an older one'}.`
    case 'unsaved':
      return `${name}'s folder has changes no turn saved yet (${listed(block.files)}). Send ${name} a message and that turn saves them.`
    case 'markers':
      return `Conflict markers are still in ${listed(block.files)} on ${name}'s branch.`
    case 'markers-unchecked':
      return `Locust could not check ${name}'s branch for conflict markers, so it will not land it yet. Read it again.`
    case 'your-changes':
      return `You have unsaved changes in ${listed(block.files)}, which this landing would overwrite. Commit them or set them aside, then read it again.`
    case 'conflicts':
      return `${listed(block.files)} ${block.files.length === 1 ? 'was' : 'were'} changed on ${target} too, in the same places.`
    case 'merging':
      return `${name}'s merge with ${target} is not finished.`
  }
}

/** The conflict a teammate can take on as a turn: begun on its branch, then asked in its conversation. */
const resolvable = (block: PublicLandBlock | undefined): boolean =>
  block?.kind === 'conflicts' || block?.kind === 'markers' || block?.kind === 'merging'

/**
 * LAND IT (0.440): the branch onto the person's, as one commit of theirs.
 *
 * Every reason it cannot land is read BEFORE the button is offered, so the
 * card says the reason in place of the button rather than failing on the
 * press. A conflict is handed to the teammate as a turn in its own
 * conversation; the landing runs again once that turn is saved.
 */
function LandCard({
  teammate,
  canEdit,
  onAsk,
  onChanged
}: {
  readonly teammate: { readonly teammateId: string; readonly name: string }
  readonly canEdit: boolean
  readonly onAsk: (prompt: string) => Promise<boolean | string>
  readonly onChanged: (said: string | undefined) => void
}): ReactElement | null {
  const [preview, setPreview] = useState<PublicLandPreview>()
  const [problem, setProblem] = useState<string>()
  const [editing, setEditing] = useState(false)
  const [message, setMessage] = useState('')
  const [working, setWorking] = useState(false)

  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined) return
    let current = true
    void bridge.landPreview(teammate.teammateId).then((answer) => {
      if (!current) return
      if (answer.ok) {
        setPreview(answer.data)
        setMessage(answer.data.draft)
      } else {
        setProblem(answer.error.message)
      }
    }).catch(() => current && setProblem('Whether it can land could not be read. Nothing was changed.'))
    return () => {
      current = false
    }
  }, [teammate.teammateId])

  const land = async (): Promise<void> => {
    const bridge = window.desktop
    if (bridge === undefined || preview === undefined) return
    setWorking(true)
    try {
      const answer = await bridge.landBranch(teammate.teammateId, message)
      if (!answer.ok) {
        setProblem(answer.error.message)
      } else if (answer.data.kind === 'landed') {
        const count = answer.data.files.length
        onChanged(
          `Landed on ${answer.data.onto} as ${answer.data.sha.slice(0, 7)}: ${String(count)} ${count === 1 ? 'file' : 'files'}, one commit of yours.`
          + (answer.data.branchReset ? ` ${teammate.name}'s branch now starts from there.` : '')
        )
        return
      } else if (answer.data.kind === 'blocked') {
        setPreview({ ...preview, block: answer.data.block })
        setEditing(false)
      } else {
        setProblem(`${answer.data.message} Your checkout is as it was.`)
      }
    } catch {
      setProblem('The landing could not be made. Your checkout is as it was.')
    } finally {
      setWorking(false)
    }
  }

  const resolve = async (): Promise<void> => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setWorking(true)
    try {
      const begun = await bridge.startResolving(teammate.teammateId)
      if (!begun.ok) {
        setProblem(begun.error.message)
        return
      }
      const files = begun.data.files.length === 0 ? 'the files that conflict' : begun.data.files.join(', ')
      const sent = await onAsk(
        `Merging ${begun.data.onto} into your branch has begun, and ${files} conflict. Resolve them: keep what both sides meant, remove every conflict marker, and change nothing else.`
      )
      if (sent === true) onChanged(`Asked ${teammate.name} to resolve ${files}. Land it when that turn ends.`)
      else setProblem(typeof sent === 'string' ? sent : `${teammate.name} could not be asked. The merge is begun on ${teammate.name}'s branch only; your checkout was not touched.`)
    } finally {
      setWorking(false)
    }
  }

  if (problem !== undefined) return <p className="lc-review__note is-warn" role="status">{problem}</p>
  if (preview === undefined || preview.block?.kind === 'nothing') return null
  const block = preview.block
  const onto = preview.onto ?? 'your branch'
  return (
    <div className="lc-land" aria-label="Land it">
      {block !== undefined ? (
        <>
          <p className={`lc-review__note${block.kind === 'busy' ? '' : ' is-warn'}`} role="status">{landBlockSentence(block, teammate.name, preview.onto)}</p>
          {resolvable(block) && (
            canEdit ? (
              <div className="lc-land__actions">
                <button type="button" className="lc-primarybutton" disabled={working} onClick={() => void resolve()}>
                  {block.kind === 'conflicts' ? `Ask ${teammate.name} to resolve` : `Ask ${teammate.name} to finish resolving`}
                </button>
              </div>
            ) : (
              <p className="lc-review__note">{teammate.name} is in a mode that cannot edit files. Switch the mode to Edit, then ask {teammate.name} to resolve it.</p>
            )
          )}
        </>
      ) : editing ? (
        <>
          <label className="lc-land__label lc-mono" htmlFor="lc-land-message">The commit on {onto}</label>
          <textarea
            id="lc-land-message"
            className="lc-land__message"
            rows={Math.min(10, Math.max(4, message.split('\n').length + 1))}
            value={message}
            onChange={(event) => setMessage(event.target.value)}
          />
          <p className="lc-review__note">
            {String(preview.files.length)} {preview.files.length === 1 ? 'file' : 'files'}, one commit made as you. Your commit hooks run; if one refuses, your checkout is left as it was.
          </p>
          <div className="lc-land__actions">
            <button type="button" className="lc-primarybutton" disabled={working || message.trim().length === 0} onClick={() => void land()}>
              {working ? 'Landing…' : 'Land it'}
            </button>
            <button type="button" className="lc-button" disabled={working} onClick={() => setEditing(false)}>
              Cancel
            </button>
          </div>
        </>
      ) : (
        <div className="lc-land__actions">
          <button type="button" className="lc-primarybutton" onClick={() => setEditing(true)}>
            Land on {onto}
          </button>
          <span className="lc-review__note">{String(preview.files.length)} {preview.files.length === 1 ? 'file' : 'files'} as one commit</span>
        </div>
      )}
    </div>
  )
}
