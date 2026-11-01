import type { ReactElement } from 'react'
import { BOT_SIZE } from '../botSizes.js'
import { useEffect, useState } from 'react'

import type { MemoryMode, MemoryScope, MemoryUpdateRequest, PublicForgottenMemory, PublicMemory, PublicTeammate } from '../../../shared/ipc.js'
import { daysUnused, lastWritten, outOfDate } from '../../../shared/memory.js'
import { MAX_ABOUT_YOU } from '../../../shared/about-you.js'
import type { AboutYouSuggestion } from '../../../shared/about-you.js'
import { TIDY_NUDGE_AT, readableReason } from '../../../shared/memory-tidy.js'
import { Icon } from './Icon.js'
import { OnTheComputer } from './OnTheComputer.js'
import { MEMORY_CLEAR_CHANNEL } from '../../../shared/ipc.js'
import { TeammateBot } from './TeammateBot.js'

/**
 * What the team remembers, and the person's hand on it.
 *
 * The shared memory Claude Code and Cursor keep, with the management both
 * of them put behind a settings page brought to the front: every memory
 * says who wrote it, in which folder, from which conversation; each can be
 * edited, switched off, or removed; one a teammate proposed waits here for
 * a keep or a forget. Only kept, switched-on memories reach a teammate.
 */
/**
 * KEEP ALL (0.372).
 *
 * A tidy pass of a folder that needed one waits as ten suggestions, and ten
 * presses is a chore that teaches a person to stop tidying (the 62-memory
 * drive). Everything kept can be put back -- a merge or a change says what
 * it was, a forgotten memory waits seven days under Recently forgotten -- so
 * keeping them all at once risks nothing a single Keep does not.
 *
 * One after another, never at once: keeping one can change what a later one
 * is about, and the store's check then refuses that one on its own
 * ("What this suggestion would change has changed since it was made"). Every
 * refusal is said, each reason once.
 */
export async function keepEvery(
  memoryIds: readonly string[],
  keep: (memoryId: string) => Promise<string | undefined>
): Promise<string | undefined> {
  const reasons: string[] = []
  for (const memoryId of memoryIds) {
    const refused = await keep(memoryId)
    if (refused !== undefined && !reasons.includes(refused)) reasons.push(refused)
  }
  return reasons.length === 0 ? undefined : reasons.join(' ')
}

export function MemoryScreen({
  memories,
  workspaceId,
  workspaceName,
  teammates,
  mode,
  onModeChange,
  onAdd,
  onUpdate,
  onRemove,
  onClear,
  forgotten = [],
  changedSince = {},
  briefTrackingSince,
  onRestore,
  onTidy,
  onOpenMission,
  notice,
  noticeWaits = false,
  onDismissNotice,
  aboutYou,
  onSaveAboutYou,
  aboutYouSuggestions = [],
  onAnswerAboutYou
}: {
  readonly memories: readonly PublicMemory[]
  readonly workspaceId: string
  readonly workspaceName: string
  readonly teammates: readonly PublicTeammate[]
  readonly mode: MemoryMode
  readonly onModeChange: (mode: MemoryMode) => void
  readonly onAdd: (text: string, scope: MemoryScope) => Promise<string | undefined>
  readonly onUpdate: (request: MemoryUpdateRequest) => Promise<string | undefined>
  readonly onRemove: (memoryId: string) => Promise<string | undefined>
  readonly onClear: (scope: 'workspace' | 'all') => Promise<string | undefined>
  /** Recently forgotten, newest first, and the way back (A1.8). */
  readonly forgotten?: readonly PublicForgottenMemory[]
  /** By memory id: the files it names that changed after it was written (A1.3). */
  readonly changedSince?: Readonly<Record<string, readonly string[]>>
  /** When counting began which memories teammates are given (A1.4). */
  readonly briefTrackingSince?: string
  readonly onRestore?: (memoryId: string) => Promise<string | undefined>
  /** A tidy pass (A1.2): opens the choice of teammate under the button. */
  readonly onTidy?: (anchor: HTMLElement) => void
  readonly onOpenMission: (missionId: string) => void
  /** The host's last word about memory, when it had one. */
  readonly notice: string | undefined
  /** The notice points at suggestions waiting below: it goes once none is waiting. */
  readonly noticeWaits?: boolean
  /**
   * Clearing it. Without this the sentence stayed for the rest of the session
   * -- its siblings on the Automations and Rooms screens both have one, and
   * this is the screen where the person is already acting on what it says.
   */
  readonly onDismissNotice: () => void
  /** About you (0.423): the person's standing note, as saved. Absent hides the card. */
  readonly aboutYou?: string
  readonly onSaveAboutYou?: (text: string) => Promise<string | undefined>
  /** Lines teammates suggested for it, and the person's answer to each (0.424). */
  readonly aboutYouSuggestions?: readonly AboutYouSuggestion[]
  readonly onAnswerAboutYou?: (id: string, add: boolean) => Promise<string | undefined>
}): ReactElement {
  const [draft, setDraft] = useState('')
  const [draftScope, setDraftScope] = useState<MemoryScope>('workspace')
  const [editing, setEditing] = useState<{ readonly memoryId: string; readonly text: string }>()
  const [error, setError] = useState<string>()
  const [busy, setBusy] = useState(false)
  const [confirmClear, setConfirmClear] = useState<'workspace' | 'all'>()

  const proposed = memories.filter((memory) => memory.status === 'proposed')
  const here = memories.filter((memory) => memory.status === 'kept' && memory.scope === 'workspace' && memory.workspaceId === workspaceId)
  const everywhere = memories.filter((memory) => memory.status === 'kept' && memory.scope === 'global')
  const elsewhere = memories.filter((memory) => memory.status === 'kept' && memory.scope === 'workspace' && memory.workspaceId !== workspaceId)

  const act = async (work: () => Promise<string | undefined>): Promise<void> => {
    setError(undefined)
    setBusy(true)
    const refused = await work()
    setBusy(false)
    if (refused !== undefined) setError(refused)
  }

  const add = async (): Promise<void> => {
    if (draft.trim().length === 0) return
    await act(async () => {
      const refused = await onAdd(draft, draftScope)
      if (refused === undefined) setDraft('')
      return refused
    })
  }

  /** Keep every suggestion waiting, one after another (0.372). */
  const keepAll = async (): Promise<void> => {
    await act(() => keepEvery(proposed.map((memory) => memory.memoryId), (memoryId) => onUpdate({ memoryId, keep: true })))
  }

  const saveEdit = async (): Promise<void> => {
    if (editing === undefined) return
    await act(async () => {
      const refused = await onUpdate({ memoryId: editing.memoryId, text: editing.text })
      if (refused === undefined) setEditing(undefined)
      return refused
    })
  }

  const when = (iso: string): string => {
    const at = new Date(iso)
    return Number.isNaN(at.getTime()) ? '' : at.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
  }

  const row = (memory: PublicMemory): ReactElement => {
    // Whoever wrote the words shown: a rewrite is credited to its writer (A1.6).
    const writer = memory.updatedBy ?? memory.by
    const author = writer.teammateId === undefined ? undefined : teammates.find((entry) => entry.teammateId === writer.teammateId)
    const isEditing = editing?.memoryId === memory.memoryId
    return (
      <div key={memory.memoryId} className={`lc-memory${memory.enabled ? '' : ' is-off'}${memory.status === 'proposed' ? ' is-proposed' : ''}`}>
        {memory.status === 'proposed' ? (
          /*
           * A suggestion has two answers, and they are its buttons. It wore
           * the kept rows' on/off switch too -- disabled, dimmed, and drawn
           * ON, a state that means nothing until it is kept (the 62-memory
           * tidy drive, 0.372). The mark the conversation's memory card uses
           * for a suggestion stands in its place, so the words still line up.
           */
          <span className="lc-memory__suggested" aria-hidden="true">
            <Icon name="spark" size={14} />
          </span>
        ) : (
          <button
            type="button"
            role="switch"
            aria-checked={memory.enabled}
            aria-label={memory.enabled ? 'Switch this memory off' : 'Switch this memory on'}
            className={`lc-memory__switch${memory.enabled ? ' is-on' : ''}`}
            disabled={busy}
            title={memory.enabled ? 'On: teammates read this' : 'Off: kept, but not read'}
            onClick={() => void act(() => onUpdate({ memoryId: memory.memoryId, enabled: !memory.enabled }))}
          >
            <span className="lc-memory__knob" />
          </button>
        )}
        <div className="lc-memory__body">
          {isEditing ? (
            <>
            <textarea
              className="lc-input lc-memory__edit"
              aria-label="Memory text"
              rows={2}
              maxLength={300}
              value={editing.text}
              onChange={(event) => setEditing({ memoryId: memory.memoryId, text: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !event.shiftKey) {
                  event.preventDefault()
                  void saveEdit()
                }
                if (event.key === 'Escape') setEditing(undefined)
              }}
            />
            <LengthLeft length={editing.text.length} />
            </>
          ) : memory.merges !== undefined ? (
            // A proposal to MERGE kept memories into one, and each as it is now (A1.2).
            <>
              <p className="lc-memory__text">
                <span className="lc-memory__change">Wants to merge these into one: </span>
                {memory.text}
              </p>
              {memory.merges.map((id) => (
                <p key={id} className="lc-memory__was">
                  Now: {memories.find((kept) => kept.memoryId === id)?.text ?? 'no longer kept'}
                </p>
              ))}
            </>
          ) : memory.forgets !== undefined ? (
            // A proposal to FORGET a kept memory: its words, what is asked, and why when it said (0.315, A1.2).
            <>
              <p className="lc-memory__text">
                <span className="lc-memory__change">Wants to forget this: </span>
                {memory.text}
              </p>
              {memory.reason !== undefined && <p className="lc-memory__was">Why: {readableReason(memory.reason, memories)}</p>}
            </>
          ) : memory.replaces !== undefined ? (
            // A proposal to CHANGE a kept memory: the new words, and the ones kept now.
            <>
              <p className="lc-memory__text">
                <span className="lc-memory__change">Wants to change this to: </span>
                {memory.text}
              </p>
              <p className="lc-memory__was">Now: {memories.find((kept) => kept.memoryId === memory.replaces)?.text ?? 'no longer kept'}</p>
            </>
          ) : (
            <p className="lc-memory__text">{memory.text}</p>
          )}
          {/*
            What it said before its last change, and the way back. The store
            has kept this one step since 0.242 and nothing showed it: a
            teammate could rewrite a memory and the person could not see
            what it had said (harness review, 2026-09-24).
          */}
          {/* A1.4: a month without being given to anyone -- worth a look, not a deletion. */}
          {!isEditing && memory.status === 'kept' && daysUnused(memory, briefTrackingSince, new Date()) !== undefined && (
            <p className="lc-memory__was">No teammate has been given this in {String(daysUnused(memory, briefTrackingSince, new Date()))} days.</p>
          )}
          {/* A1.3: a file it names changed after it was written. */}
          {!isEditing && (changedSince[memory.memoryId] ?? []).length > 0 && (
            <p className="lc-memory__was lc-memory__stale">May be out of date: {outOfDate(changedSince[memory.memoryId] ?? [])}.</p>
          )}
          {!isEditing && memory.previousText !== undefined && memory.previousText !== memory.text && (
            <p className="lc-memory__was">
              Was: {memory.previousText}{' '}
              <button
                type="button"
                className="lc-linkbutton"
                disabled={busy}
                onClick={() => void act(() => onUpdate({ memoryId: memory.memoryId, text: memory.previousText! }))}
              >
                Put it back
              </button>
            </p>
          )}
          <p className="lc-memory__meta lc-mono">
            {author === undefined ? (
              <span>{writer.name}</span>
            ) : (
              <span className="lc-memory__author">
                <TeammateBot hue={author.hue} avatar={author.avatar} size={BOT_SIZE.memoryAuthor} activity="idle" presence="none" />
                {author.name}
              </span>
            )}
            <span> · {memory.scope === 'global' ? `everywhere, from ${memory.workspaceName}` : memory.workspaceName}</span>
            {/* When it last changed: a memory rewritten today is today's. */}
            <span> · {memory.updatedAt === undefined ? when(memory.createdAt) : `changed ${when(lastWritten(memory))}`}</span>
            {memory.missionId !== undefined && (
              <>
                <span> · </span>
                <button type="button" className="lc-linkbutton" onClick={() => onOpenMission(memory.missionId!)}>
                  Open the conversation
                </button>
              </>
            )}
          </p>
        </div>
        <span className="lc-memory__actions">
          {memory.status === 'proposed' ? (
            <>
              {/* Keep applies the proposal; the other button drops only the proposal. */}
              <button type="button" className="lc-button is-active" disabled={busy} onClick={() => void act(() => onUpdate({ memoryId: memory.memoryId, keep: true }))}>
                {memory.merges !== undefined ? 'Merge them' : memory.forgets !== undefined ? 'Forget it' : memory.replaces !== undefined ? 'Keep the change' : 'Keep'}
              </button>
              <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => void act(() => onRemove(memory.memoryId))}>
                {memory.merges !== undefined ? 'Keep them apart' : memory.forgets !== undefined ? 'Keep it' : memory.replaces !== undefined ? 'Keep the old one' : 'Forget'}
              </button>
            </>
          ) : isEditing ? (
            <>
              <button type="button" className="lc-button is-active" disabled={busy || editing.text.trim().length === 0} onClick={() => void saveEdit()}>
                Save
              </button>
              <button type="button" className="lc-ghostbutton" onClick={() => setEditing(undefined)}>
                Cancel
              </button>
            </>
          ) : (
            <>
              <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => setEditing({ memoryId: memory.memoryId, text: memory.text })}>
                Edit
              </button>
              <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => void act(() => onRemove(memory.memoryId))}>
                Remove
              </button>
            </>
          )}
        </span>
      </div>
    )
  }

  /*
   * A line for the person's own About-you note (0.424): the same register as
   * a suggested memory, and two answers -- add it to the note, or let it go.
   */
  const suggestionRow = (suggestion: AboutYouSuggestion): ReactElement => (
    <div key={suggestion.id} className="lc-memory is-proposed">
      <span className="lc-memory__suggested" aria-hidden="true">
        <Icon name="spark" size={14} />
      </span>
      <div className="lc-memory__body">
        <p className="lc-memory__text">
          <span className="lc-memory__change">Suggested for About you: </span>
          {suggestion.text}
        </p>
        <p className="lc-memory__meta lc-mono">
          <span>{suggestion.by}</span>
          <span> · {when(suggestion.at)}</span>
        </p>
      </div>
      <span className="lc-memory__actions">
        <button type="button" className="lc-button is-active" disabled={busy || onAnswerAboutYou === undefined} onClick={() => void act(() => onAnswerAboutYou!(suggestion.id, true))}>
          Add to About you
        </button>
        <button type="button" className="lc-ghostbutton" disabled={busy || onAnswerAboutYou === undefined} onClick={() => void act(() => onAnswerAboutYou!(suggestion.id, false))}>
          Dismiss
        </button>
      </span>
    </div>
  )

  const kept = here.length + everywhere.length + elsewhere.length

  return (
    <div className="lc-screen">
      <div className="lc-screen__header">
        <span className="lc-screen__title">Memory</span>
        <span className="lc-screen__meta lc-mono">
          {kept === 0 ? 'nothing remembered yet' : `${String(kept)} remembered`}
          {proposed.length + aboutYouSuggestions.length > 0 ? ` · ${String(proposed.length + aboutYouSuggestions.length)} waiting for you` : ''}
          {mode === 'off' ? ' · off' : ''}
        </span>
      </div>
      <div className="lc-screen__scroll">
        {notice !== undefined && notice.length > 0 && !(noticeWaits && proposed.length === 0 && aboutYouSuggestions.length === 0) && (
          <p className="lc-settings__note lc-memory__notice">
            {notice}{' '}
            <button type="button" className="lc-ghostbutton" onClick={onDismissNotice}>
              Dismiss
            </button>
          </p>
        )}
        {error !== undefined && <p className="lc-dialog__error">{error}</p>}

        {/*
          * What needs an answer, first. It sat under "How memory is kept" -- a
          * paragraph and the mode switch -- so a person sent here by "answer
          * it on the Memory screen" arrived at the explanation and scrolled
          * for the thing they came to do (drive-memory-tidy, 0.371).
          */}
        {(proposed.length > 0 || aboutYouSuggestions.length > 0) && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Waiting for you</h2>
            {proposed.length >= 2 && (
              <div className="lc-memoryform__row">
                <span className="lc-settings__note">Anything you keep can be put back.</span>
                <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => void keepAll()}>
                  Keep all {proposed.length}
                </button>
              </div>
            )}
            <div className="lc-memorylist">
              {aboutYouSuggestions.map(suggestionRow)}
              {proposed.map(row)}
            </div>
          </section>
        )}

        {aboutYou !== undefined && onSaveAboutYou !== undefined && (
          <AboutYouCard saved={aboutYou} onSave={onSaveAboutYou} />
        )}

        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">How memory is kept</h2>
          <p className="lc-settings__lede">
            Every teammate in a folder reads what is remembered for that folder and everything marked
            everywhere. A teammate writes a memory by ending a reply with one; you can also write one
            below. Nothing here is uploaded: memory lives in Locust&rsquo;s own data on this machine.
          </p>
          <div className="lc-segmented" role="radiogroup" aria-label="How a teammate's memory is treated">
            {(
              [
                ['auto', 'Keep and tell me'],
                ['ask', 'Ask me first'],
                ['off', 'Off']
              ] as const
            ).map(([value, label]) => (
              <button key={value} type="button" role="radio" aria-checked={mode === value} className={`lc-button${mode === value ? ' is-active' : ''}`} onClick={() => onModeChange(value)}>
                {label}
              </button>
            ))}
          </div>
          <p className="lc-settings__note">
            {mode === 'auto'
              ? 'A memory a teammate writes is kept at once and said in the conversation; undo it here.'
              : mode === 'ask'
                ? 'A memory a teammate writes, changes or forgets waits below until you answer it.'
                : 'Teammates are not told what is remembered and cannot write memory. What is kept stays here.'}
          </p>
        </section>

        <section className="lc-settings__section">
          {/* The folder's own name keeps its own spelling: titles are set in
            * capitals, and a name is an identifier, not a title. */}
          <h2 className="lc-settings__heading">
            This folder · <span className="lc-title__name">{workspaceName}</span>
          </h2>
          {/* A tidy pass (A1.2): a teammate reads them and suggests; nothing changes until it is kept. */}
          {onTidy !== undefined && mode !== 'off' && here.length >= 2 && (
            // What it does, then the control -- the way a settings row reads.
            // (A ghost button pushes itself right; first in the row, it sat
            // indented under the heading.)
            <div className="lc-memoryform__row">
              <span className="lc-settings__note">
                {here.length > TIDY_NUDGE_AT ? `${String(here.length)} memories here. ` : ''}A teammate reads them and suggests merges and retirements; nothing changes until you keep it.
              </span>
              <button type="button" className="lc-ghostbutton" disabled={busy} onClick={(event) => onTidy(event.currentTarget)}>
                Tidy up…
              </button>
            </div>
          )}
          {here.length === 0 ? <p className="lc-settings__note">Nothing remembered for this folder yet.</p> : <div className="lc-memorylist">{here.map(row)}</div>}
        </section>

        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Everywhere</h2>
          {everywhere.length === 0 ? <p className="lc-settings__note">Nothing remembered for every folder yet.</p> : <div className="lc-memorylist">{everywhere.map(row)}</div>}
        </section>

        {elsewhere.length > 0 && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Other folders</h2>
            <p className="lc-settings__note">Remembered for folders Locust has been opened in. Teammates here do not read these.</p>
            <div className="lc-memorylist">{elsewhere.map(row)}</div>
          </section>
        )}

        {/*
          Recently forgotten (A1.8). A forget -- the person's, a teammate's,
          Forget everything -- used to be final the moment it landed; every
          kept memory that goes is held here for 7 days with the way back.
        */}
        {forgotten.length > 0 && onRestore !== undefined && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Recently forgotten</h2>
            <p className="lc-settings__note">Kept for 7 days, then gone for good. Teammates do not read these.</p>
            <div className="lc-memorylist">
              {forgotten.map((entry) => (
                <div key={`${entry.memory.memoryId}-${entry.forgottenAt}`} className="lc-memory is-forgotten">
                  <div className="lc-memory__body">
                    <p className="lc-memory__text">{entry.memory.text}</p>
                    <p className="lc-memory__meta lc-mono">
                      <span>forgotten by {entry.forgottenBy.name}</span>
                      <span> · {when(entry.forgottenAt)}</span>
                      <span> · {entry.memory.scope === 'global' ? 'everywhere' : entry.memory.workspaceName}</span>
                    </p>
                  </div>
                  <span className="lc-memory__actions">
                    <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => void act(() => onRestore(entry.memory.memoryId))}>
                      Restore
                    </button>
                  </span>
                </div>
              ))}
            </div>
          </section>
        )}

        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Remember something</h2>
          <div className="lc-memoryform">
            <textarea
              className="lc-input"
              aria-label="What to remember"
              placeholder="One sentence, specific enough to act on"
              rows={2}
              maxLength={300}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
            />
            <LengthLeft length={draft.length} />
            <div className="lc-memoryform__row">
              <div className="lc-segmented" role="radiogroup" aria-label="Where it applies">
                {(
                  [
                    ['workspace', 'This folder'],
                    ['global', 'Everywhere']
                  ] as const
                ).map(([value, label]) => (
                  <button key={value} type="button" role="radio" aria-checked={draftScope === value} className={`lc-button${draftScope === value ? ' is-active' : ''}`} onClick={() => setDraftScope(value)}>
                    {label}
                  </button>
                ))}
              </div>
              <button type="button" className="lc-primarybutton" disabled={busy || draft.trim().length === 0} onClick={() => void add()}>
                Remember
              </button>
            </div>
          </div>
        </section>

        {kept + proposed.length > 0 && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Forget</h2>
            {/* Forgetting them all at once is done on the computer (0.849); one memory at a time is forgotten anywhere. */}
            <OnTheComputer channel={MEMORY_CLEAR_CHANNEL}>
            {confirmClear === undefined ? (
              <div className="lc-memoryform__row lc-memoryform__row--start">
                <button type="button" className="lc-ghostbutton" onClick={() => setConfirmClear('workspace')}>
                  Forget everything for this folder
                </button>
                <button type="button" className="lc-ghostbutton" onClick={() => setConfirmClear('all')}>
                  Forget everything
                </button>
              </div>
            ) : (
              <div className="lc-memoryform__row">
                <span className="lc-settings__note">
                  {confirmClear === 'all' ? 'Every memory, every folder, forgotten. ' : `Everything remembered for ${workspaceName}, forgotten. `}
                  {/* Honest since A1.8: they are held, not gone. */}
                  You can restore them from Recently forgotten for 7 days.
                </span>
                <button
                  type="button"
                  className="lc-button is-active"
                  disabled={busy}
                  onClick={() => {
                    const scope = confirmClear
                    setConfirmClear(undefined)
                    void act(() => onClear(scope))
                  }}
                >
                  Yes, forget
                </button>
                <button type="button" className="lc-ghostbutton" onClick={() => setConfirmClear(undefined)}>
                  Cancel
                </button>
              </div>
            )}
            </OnTheComputer>
          </section>
        )}
      </div>
    </div>
  )
}

/**
 * About you (0.423): the person's own standing note, read by every teammate
 * before every run (shared/about-you.ts). Written here and nowhere else; a
 * teammate never changes it.
 */
function AboutYouCard({
  saved,
  onSave
}: {
  readonly saved: string
  readonly onSave: (text: string) => Promise<string | undefined>
}): ReactElement {
  const [text, setText] = useState(saved)
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<string | undefined>(undefined)
  // A refusal is not a save: it reads in the colour of what needs you (0.433).
  const [refused, setRefused] = useState(false)
  // What was saved elsewhere (a relaunch, a second window) replaces an untouched draft.
  useEffect(() => {
    setText(saved)
  }, [saved])
  const changed = text.trim() !== saved.trim()
  const left = MAX_ABOUT_YOU - text.length
  const save = async (): Promise<void> => {
    setBusy(true)
    const error = await onSave(text.trim())
    setBusy(false)
    setRefused(error !== undefined)
    setSaid(error ?? (text.trim().length === 0 ? 'Removed. Teammates are no longer given a note about you.' : 'Saved. Every teammate reads this from their next run.'))
  }
  return (
    <section className="lc-settings__section lc-aboutyou">
      <h2 className="lc-settings__heading">About you</h2>
      <p className="lc-settings__lede">
        Every teammate reads this before each run, on every runtime — how you like to work, what to always or never do.
        Only you change it.
      </p>
      <div className="lc-memoryform">
        <textarea
          className="lc-input lc-aboutyou__text"
          aria-label="About you"
          placeholder="For example: I read diffs, not long explanations. Keep answers short. Ask before deleting anything."
          rows={4}
          maxLength={MAX_ABOUT_YOU}
          value={text}
          onChange={(event) => {
            setText(event.target.value)
            setSaid(undefined)
            setRefused(false)
          }}
        />
        <div className="lc-memoryform__row">
          <span className={`lc-settings__note${refused ? ' lc-tone-amber' : ''}`} role="status">
            {said ?? (left < 200 ? `${String(left)} characters left` : '')}
          </span>
          <button type="button" className="lc-primarybutton" disabled={busy || !changed} onClick={() => void save()}>
            {text.trim().length === 0 && saved.length > 0 ? 'Remove' : 'Save'}
          </button>
        </div>
      </div>
    </section>
  )
}

/*
 * THE LIMIT, SHOWN NEAR IT (QA-2026-09-29 round 2, N6). A memory stops at
 * 300 characters, and the box simply stopped taking keys with nothing to say
 * why. Shown from 240, where it starts to matter.
 */
function LengthLeft({ length }: { readonly length: number }): ReactElement | null {
  if (length < 240) return null
  return (
    <span className={`lc-memorycount lc-mono${length >= 300 ? ' is-full' : ''}`} aria-live="polite">
      {length >= 300 ? '300 of 300: a memory is one or two sentences' : `${String(length)} of 300`}
    </span>
  )
}
