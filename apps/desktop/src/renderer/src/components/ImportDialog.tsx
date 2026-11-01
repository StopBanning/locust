import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'
import { useModal } from '../useModal.js'

import type { ImportableSessionView, SessionImportListResponse, SessionImportResponse } from '../../../shared/ipc.js'
import { RuntimeMark } from './RuntimeMark.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'

/**
 * IMPORT A CONVERSATION (main/session-import.ts).
 *
 * The sessions a person had in Claude Code, Codex, OpenCode or Cursor Agent (0.799) in the last 30 days,
 * newest first, each with its folder and when it was last used. Choosing one
 * brings its exchanges in as a conversation; the next message there resumes
 * the same session, in its own folder. A session written to in the last two
 * minutes is probably still open in a terminal, and says so: two programs
 * writing one session interleave it.
 */
export function ImportDialog({
  onList,
  onImport,
  onImported,
  onCancel,
  now = () => new Date()
}: {
  readonly onList: () => Promise<SessionImportListResponse>
  readonly onImport: (session: ImportableSessionView) => Promise<SessionImportResponse>
  /** The newest turn of the conversation just made, to open it. */
  readonly onImported: (missionId: string) => void
  readonly onCancel: () => void
  readonly now?: () => Date
}): ReactElement {
  const [sessions, setSessions] = useState<readonly ImportableSessionView[] | undefined>(undefined)
  const [problem, setProblem] = useState<string>()
  const [importing, setImporting] = useState<string>()
  const [query, setQuery] = useState('')
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onCancel)
  useEffect(() => {
    let live = true
    void onList().then((answer) => {
      if (!live) return
      if (answer.ok) {
        setSessions(answer.sessions)
        if (answer.warnings?.length) setProblem(answer.warnings.join(' '))
      }
      else {
        setSessions([])
        setProblem(answer.message)
      }
    }).catch(() => {
      if (live) {
        setSessions([])
        setProblem('The sessions could not be listed. Nothing was changed; close this and open it again.')
      }
    })
    return () => {
      live = false
    }
  }, [])
  const shown = (sessions ?? []).filter((session) => {
    const words = query.trim().toLowerCase()
    return words.length === 0 || `${session.title} ${session.folderName}`.toLowerCase().includes(words)
  })
  const bring = async (session: ImportableSessionView): Promise<void> => {
    setImporting(session.sessionId)
    setProblem(undefined)
    const answer = await onImport(session).catch(() => ({ ok: false, message: 'It could not be brought in.' }) as const)
    setImporting(undefined)
    if (answer.ok) onImported(answer.missionId)
    else setProblem(answer.message)
  }
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-importdialog" role="dialog" aria-modal="true" aria-label="Import a conversation">
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Import a conversation</span>
          <span className="lc-dialog__sub">the last 30 days</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onCancel}>
            ×
          </button>
        </div>
        <div className="lc-dialog__body lc-importdialog__body">
          <input
            className="lc-input lc-importdialog__search"
            type="text"
            placeholder="Search by title or folder"
            aria-label="Search sessions"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          {problem !== undefined && <p className="lc-importdialog__problem" role="alert">{problem}</p>}
          {sessions === undefined ? (
            <p className="lc-importdialog__empty">Looking for your Claude Code, Codex, OpenCode and Cursor Agent sessions…</p>
          ) : shown.length === 0 ? (
            <p className="lc-importdialog__empty">
              {sessions.length === 0
                ? 'No sessions from the last 30 days to bring in. Ones already in Locust are not listed.'
                : 'Nothing matches that.'}
            </p>
          ) : (
            <ul className="lc-importdialog__list">
              {shown.map((session) => (
                <li key={`${session.runtime}:${session.sessionId}`} className="lc-importdialog__row">
                  <RuntimeMark runtime={session.runtime} size={14} />
                  <span className="lc-importdialog__what">
                    <span className="lc-importdialog__title">{session.title}</span>
                    <span className="lc-importdialog__meta lc-mono" title={session.cwd}>
                      {runtimeDisplayName(session.runtime)} · {session.folderName} · {ageOf(session.updatedAt, now())}
                      {session.folderMissing ? ' · folder no longer available' : ''}
                      {session.openNow ? ' · probably still open in a terminal' : ''}
                    </span>
                  </span>
                  <button
                    type="button"
                    className="lc-button"
                    disabled={importing !== undefined || session.folderMissing === true}
                    title={session.folderMissing ? 'Restore its folder and open this list again to continue this conversation.' : session.openNow ? 'It was written to in the last two minutes. Close it in the terminal first, or both will write to it.' : 'Bring it in. Your next message there continues this same session.'}
                    onClick={() => void bring(session)}
                  >
                    {importing === session.sessionId ? 'Importing…' : 'Import'}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="lc-dialog__foot">
          <span className="lc-importdialog__note">Only the words come in; the steps it took stay in the session. Your next message continues it in its own folder.</span>
          <button type="button" className="lc-button" onClick={onCancel}>Close</button>
        </div>
      </div>
    </div>
  )
}

/** "3m", "5h", "2d": as the sidebar says ages. */
export function ageOf(iso: string, now: Date): string {
  const minutes = Math.max(0, Math.round((now.getTime() - Date.parse(iso)) / 60_000))
  if (minutes < 60) return `${String(Math.max(1, minutes))}m`
  const hours = Math.round(minutes / 60)
  if (hours < 48) return `${String(hours)}h`
  return `${String(Math.round(hours / 24))}d`
}
