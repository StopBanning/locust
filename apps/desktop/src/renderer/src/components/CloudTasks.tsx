import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { ClaudeCloudReading, PublicClaudeCloudSession, PublicCloudFolder, PublicCloudTask, PublicCloudWhere } from '../../../shared/ipc.js'
import { fileCounts, parseUnifiedDiff } from '../diff.js'
import { agoLabel } from '../teammateWork.js'
import { cloudUpdatedLabel, startCloudWatch } from '../cloudWatch.js'
import { DiffView } from './DiffView.js'
import { Icon } from './Icon.js'
import { AgentText } from './ThreadItems.js'
import { LineCounts } from './LineCounts.js'

/**
 * CLOUD TASKS (0.503): what was handed to Codex Cloud from this folder, how
 * each is going, and its change -- shown, and brought home only on Apply.
 * Beside the conversation, as the side question and the review are: the
 * cloud's work is not a turn of this conversation, and it can outlast it.
 */
export function CloudTasks({
  tasks,
  where,
  notes,
  problem,
  applying,
  onShowChange,
  onApply,
  onOpen,
  onClose,
  folders,
  onOpenFolder,
  onChooseFolder,
  claude
}: {
  readonly tasks: readonly PublicCloudTask[]
  readonly where: PublicCloudWhere | undefined
  /** What the cloud will not see, said when the newest task started. */
  readonly notes: readonly string[]
  readonly problem: string | undefined
  readonly applying: string | undefined
  readonly onShowChange: (taskId: string) => Promise<string | undefined>
  readonly onApply: (taskId: string) => void
  readonly onOpen: (url: string) => void
  readonly onClose: () => void
  /** The person's folders that ARE on GitHub, offered when this one is not (0.504). */
  /** Undefined while Locust is still asking which of them have a cloud environment. */
  readonly folders: readonly PublicCloudFolder[] | undefined
  readonly onOpenFolder: (id: string) => void
  readonly onChooseFolder: () => void
  /**
   * Claude's cloud (0.538), when a Claude model is picked or a session was
   * sent from this folder. Claude Code says nothing Locust can follow, so a
   * session is listed as sent, with where to see it and how to bring it home.
   */
  readonly claude?: {
    readonly picked: boolean
    readonly sessions: readonly PublicClaudeCloudSession[]
    readonly note: string | undefined
    readonly onHome: (id: string) => void
    readonly onForget: (id: string) => void
    /** The session's own page when Locust knows it, otherwise the list of sessions. */
    readonly onOpenWeb: (url?: string) => void
    /** A follow-up to the session; resolves to what went wrong, or undefined when sent. */
    readonly onSend: (id: string, message: string) => Promise<string | undefined>
    /** What the session said and changed, brought in out of sight (0.558). */
    readonly onCheck: (id: string) => Promise<ClaudeCloudReading>
    /** Its change into this folder; resolves to what went wrong, or undefined when applied. */
    readonly onApply: (id: string) => Promise<string | undefined>
    readonly onContinue: (id: string) => Promise<string | undefined>
  }
}): ReactElement {
  // Codex's warnings are about Codex Cloud: not said while Claude's is the one picked, unless Codex tasks are here.
  const codexSide = claude?.picked !== true || tasks.length > 0
  const known = folders ?? []
  const [open, setOpen] = useState<{ readonly taskId: string; readonly diff: string | undefined; readonly loading: boolean }>()
  const newestFirst = [...tasks].sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt))
  const show = (taskId: string): void => {
    if (open?.taskId === taskId) {
      setOpen(undefined)
      return
    }
    setOpen({ taskId, diff: undefined, loading: true })
    void onShowChange(taskId).then((diff) => setOpen((current) => (current?.taskId === taskId ? { taskId, diff, loading: false } : current)))
  }
  return (
    <aside className="lc-viewer lc-beside lc-cloudtasks" aria-label="Cloud tasks">
      <div className="lc-viewer__head">
        <Icon name="cloud" size={14} />
        <span className="lc-beside__who">
          <span className="lc-viewer__name">Cloud tasks</span>
          <span className="lc-beside__title lc-mono">
            {claude?.picked === true ? 'Claude’s cloud' : where?.repo === undefined ? 'Codex Cloud' : `${where.repo}${where.branch === undefined ? '' : ` · ${where.branch}`}`}
          </span>
        </span>
        <span className="lc-viewer__spacer" />
        <button type="button" className="lc-viewer__close" aria-label="Close" title="Close" onClick={onClose}>
          <Icon name="close" size={13} />
        </button>
      </div>
      <div className="lc-beside__thread lc-cloudtasks__list">
        {problem !== undefined && <p className="lc-cloudtasks__problem" role="alert">{problem}</p>}
        {claude !== undefined && (claude.picked || claude.sessions.length > 0) && (
          <ClaudeCloudSessions {...claude} />
        )}
        {/*
          * NOT ON GITHUB, SAID WITH THE WAY ON (0.504). Colin, at the old
          * refusal: "i have no idea what folder the cloud is in". The folder
          * is named, what the cloud needs is said, and the person's folders
          * that are on GitHub are one click away.
          */}
        {/*
          * CLAUDE'S CLOUD, TOO (0.545). Colin's first send from .claude went
          * through, and the session answered that /home/user "isn't a git
          * repository": Claude's cloud works on a GitHub repository, and
          * without one it starts with no project. It still runs, so it is
          * said, not refused.
          */}
        {(codexSide || claude?.picked === true) && where !== undefined && where.repo === undefined && (
          <div className="lc-cloudtasks__elsewhere">
            {claude?.picked === true && !codexSide ? (
              <p className="lc-cloudtasks__note">
                <strong>{where.folderName ?? 'This folder'}</strong> is not on GitHub. Claude’s cloud works on a copy of a
                GitHub repository, so a task sent from here starts with no project. Open a folder that is on GitHub to work on it there.
              </p>
            ) : (
              <p className="lc-cloudtasks__note">
                <strong>{where.folderName ?? 'This folder'}</strong> is not on GitHub. A cloud task runs on a copy of a GitHub
                repository in Codex Cloud, not on this computer, so it needs a folder that is on GitHub.
              </p>
            )}
            {folders === undefined ? (
              <p className="lc-cloudtasks__note">Checking your other folders…</p>
            ) : known.length > 0 ? (
              <>
                <p className="lc-cloudtasks__note">Your folders that are:</p>
                <ul className="lc-cloudtasks__folders">
                  {known.map((folder) => (
                    <li key={folder.id}>
                      <span className="lc-cloudtasks__foldername">{folder.name}</span>
                      <span className="lc-cloudtasks__repo lc-mono">{folder.environment === 'ready' || !codexSide ? folder.repo : `${folder.repo} · no cloud environment`}</span>
                      <button type="button" className="lc-button" onClick={() => onOpenFolder(folder.id)}>Open</button>
                    </li>
                  ))}
                </ul>
              </>
            ) : (
              <p className="lc-cloudtasks__note">None of the folders Locust knows is on GitHub yet. Push this project to GitHub, or open one that is.</p>
            )}
            <button type="button" className="lc-button" onClick={onChooseFolder}>
              <Icon name="folder" size={13} /> Choose a folder…
            </button>
          </div>
        )}
        {codexSide && notes.map((note) => (
          <p key={note} className="lc-cloudtasks__note">{note}</p>
        ))}
        {/*
          * ON GITHUB, BUT NO CLOUD ENVIRONMENT (0.505). Colin's brief went
          * to Codex Cloud from his Locust folder and came back refused: the
          * repository was right, it had no environment. Said before a send,
          * with how to make one, and the folders that have one.
          */}
        {codexSide && where?.repo !== undefined && where.environment === 'missing' && (
          <div className="lc-cloudtasks__elsewhere">
            <p className="lc-cloudtasks__note">
              <strong>{where.repo}</strong> has no Codex Cloud environment yet, so a task sent from here would be refused. Make
              one in the Codex app: Settings &gt; Legacy Codex Cloud, pick this repository, then Save and publish.
            </p>
            {folders === undefined && <p className="lc-cloudtasks__note">Checking your other folders for one that has an environment…</p>}
            {known.some((folder) => folder.environment === 'ready') && (
              <>
                <p className="lc-cloudtasks__note">Or use a folder that has one:</p>
                <ul className="lc-cloudtasks__folders">
                  {known.filter((folder) => folder.environment === 'ready').map((folder) => (
                    <li key={folder.id}>
                      <span className="lc-cloudtasks__foldername">{folder.name}</span>
                      <span className="lc-cloudtasks__repo lc-mono">{folder.repo}</span>
                      <button type="button" className="lc-button" onClick={() => onOpenFolder(folder.id)}>Open</button>
                    </li>
                  ))}
                </ul>
              </>
            )}
            <button type="button" className="lc-button" onClick={onChooseFolder}>
              <Icon name="folder" size={13} /> Choose a folder…
            </button>
          </div>
        )}
        {codexSide && newestFirst.length === 0 && where?.repo !== undefined && where.environment !== 'missing' && (
          <p className="lc-cloudtasks__empty">
            Pick Cloud in the chat-type menu and describe a task. It runs in Codex Cloud on this repository, as GitHub has
            it; its change stays there until you apply it here.
          </p>
        )}
        {newestFirst.map((task) => {
          const state = task.status.state
          const counts = task.status.added === undefined ? undefined : `+${String(task.status.added)} −${String(task.status.removed ?? 0)} in ${String(task.status.files ?? 0)} ${task.status.files === 1 ? 'file' : 'files'}`
          const line = state === 'pending'
            ? `Working in the cloud · started ${agoLabel(task.createdAt) ?? 'just now'}`
            : state === 'ready'
              ? `Ready${counts === undefined ? '' : ` · ${counts}`}`
              : state === 'applied'
                ? `Applied to this folder${task.appliedAt === undefined ? '' : ` ${agoLabel(task.appliedAt) ?? ''}`}${counts === undefined ? '' : ` · ${counts}`}`
                : state === 'failed'
                  ? 'The cloud could not finish it. Open it in Codex to see why.'
                  : 'Codex has not said how it is going yet.'
          const showing = open?.taskId === task.taskId
          const files = showing && open?.diff !== undefined ? parseUnifiedDiff(open.diff) : []
          return (
            <section key={task.taskId} className={`lc-cloudtask is-${state}`} aria-label={task.status.title ?? task.prompt}>
              <div className="lc-cloudtask__prompt">{task.status.title ?? task.prompt}</div>
              <div className="lc-cloudtask__state">
                {state === 'pending' && <span className="lc-cloudtask__dot" aria-hidden="true" />}
                <span>{line}</span>
              </div>
              <div className="lc-cloudtask__actions">
                {(state === 'ready' || state === 'applied') && (
                  <button type="button" className="lc-button" aria-expanded={showing} onClick={() => show(task.taskId)}>
                    <Icon name="diff" size={13} /> {showing ? 'Hide the change' : 'Show the change'}
                  </button>
                )}
                {state === 'ready' && (
                  <button type="button" className="lc-primarybutton" disabled={applying !== undefined} onClick={() => onApply(task.taskId)} title="Its change comes into this folder, not committed, so you can look before you keep it">
                    {applying === task.taskId ? 'Applying…' : 'Apply to this folder'}
                  </button>
                )}
                <button type="button" className="lc-button" onClick={() => onOpen(task.url)} title="The task in Codex, in your browser">
                  Open in Codex
                </button>
              </div>
              {showing && (
                <div className="lc-review__files">
                  {open?.loading === true && <p className="lc-cloudtasks__note">Reading the change…</p>}
                  {open?.loading === false && open.diff === undefined && <p className="lc-cloudtasks__note">Codex did not return a change for it.</p>}
                  {files.map((file) => {
                    const fileCountsNow = fileCounts(file)
                    return (
                      <section key={`${task.taskId}:${file.path}`} aria-label={file.path}>
                        <div className="lc-review__path lc-mono">
                          <span className="lc-review__name">{file.path}</span>
                          {file.status !== 'MODIFIED' && <span className="lc-review__status">{file.status.toLowerCase()}</span>}
                          <LineCounts added={fileCountsNow.added} removed={fileCountsNow.removed} />
                        </div>
                        <DiffView file={file} truncated={false} reported={undefined} />
                      </section>
                    )
                  })}
                </div>
              )}
            </section>
          )
        })}
      </div>
    </aside>
  )
}


/**
 * What was handed to Claude's cloud from this folder (0.538).
 *
 * 0.556: a session Locust started out of sight carries Claude Code's own id,
 * so it opens on claude.ai itself, takes a follow-up from here, and comes
 * home by name. What it SAYS is not readable from here -- Claude Code does
 * not let this account attach to a cloud session -- so the row says where
 * to read it rather than pretending to follow it.
 */
function ClaudeCloudSessions({
  picked,
  sessions,
  note,
  onHome,
  onForget,
  onOpenWeb,
  onSend,
  onCheck,
  onApply,
  onContinue
}: NonNullable<Parameters<typeof CloudTasks>[0]['claude']>): ReactElement {
  return (
    <div className="lc-cloudtasks__claude">
      {(picked || sessions.length === 0) && (
        <p className="lc-cloudtasks__empty">
          Describe a task and send it. Claude does the work on Anthropic’s machines, not this computer. Show what it
          did reads its conversation and its change here; Apply brings the change into this folder. If Claude Code
          needs to ask you something first, such as whether you trust this folder, it opens a window for that.
        </p>
      )}
      {note !== undefined && <p className="lc-cloudtasks__note" role="status">{note}</p>}
      {sessions.map((session) => (
        <ClaudeCloudRow key={session.id} session={session} onHome={onHome} onForget={onForget} onOpenWeb={onOpenWeb} onSend={onSend} onCheck={onCheck} onApply={onApply} onContinue={onContinue} />
      ))}
    </div>
  )
}

function ClaudeCloudRow({
  session,
  onHome,
  onForget,
  onOpenWeb,
  onSend,
  onCheck,
  onApply,
  onContinue
}: {
  readonly session: PublicClaudeCloudSession
} & Pick<NonNullable<Parameters<typeof CloudTasks>[0]['claude']>, 'onHome' | 'onForget' | 'onOpenWeb' | 'onSend' | 'onCheck' | 'onApply' | 'onContinue'>): ReactElement {
  const [draft, setDraft] = useState('')
  const [sending, setSending] = useState(false)
  const [said, setSaid] = useState<string>()
  const [reading, setReading] = useState<ClaudeCloudReading>()
  const [checking, setChecking] = useState(false)
  const [applying, setApplying] = useState(false)
  const [applied, setApplied] = useState(false)
  const [openingTerminal, setOpeningTerminal] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [watching, setWatching] = useState(false)
  const [clock, setClock] = useState(Date.now())
  const readingRef = useRef(reading)
  const pending = useRef<Promise<ClaudeCloudReading> | undefined>(undefined)
  const readOnce = (): Promise<ClaudeCloudReading> => {
    if (pending.current !== undefined) return pending.current
    setChecking(true)
    setSaid(undefined)
    const request = onCheck(session.id).catch(() => ({ ok: false as const, message: 'Claude Code could not be reached to read it. Check again.' })).then((answer) => {
      setChecking(false)
      setApplied(false)
      setReading(answer)
      readingRef.current = answer
      pending.current = undefined
      return answer
    })
    pending.current = request
    return request
  }
  const readOnceRef = useRef(readOnce)
  readOnceRef.current = readOnce
  useEffect(() => {
    if (!watching || !expanded) return
    return startCloudWatch({
      check: () => readOnceRef.current(),
      ...(readingRef.current === undefined ? {} : { initial: readingRef.current }),
      onReading: () => undefined,
      onStopped: (reason) => {
        setWatching(false)
        setSaid(reason === 'time' ? 'Watch stopped after 30 minutes.' : reason === 'unchanged' ? 'Watch stopped after two reads with nothing new.' : 'Watch stopped because the session could not be read. Check again; the last reading stays here.')
      }
    })
  }, [watching, expanded, session.id])
  useEffect(() => {
    if (!expanded) return
    setClock(Date.now())
    const timer = setInterval(() => setClock(Date.now()), 60_000)
    return () => clearInterval(timer)
  }, [expanded])
  const check = (): void => { setExpanded(true); void readOnce() }
  const apply = (): void => {
    if (applying) return
    setWatching(false)
    setApplying(true)
    void onApply(session.id).then((problem) => {
      setApplying(false)
      if (problem === undefined) {
        setApplied(true)
        setSaid('Its change is in this folder, not committed. Look it over before you keep it.')
      } else {
        setSaid(problem)
      }
    })
  }
  const read = reading?.ok === true ? reading : undefined
  const files = read?.diff === undefined ? [] : parseUnifiedDiff(read.diff)
  const known = session.sessionId !== undefined
  const send = (): void => {
    const text = draft.trim()
    if (text.length === 0 || sending) return
    setSending(true)
    setSaid(undefined)
    void onSend(session.id, text).then((problem) => {
      setSending(false)
      if (problem === undefined) {
        setDraft('')
        setSaid('Sent. Check again in a moment to read its reply.')
      } else {
        setSaid(problem)
      }
    })
  }
  return (
    <section className="lc-cloudtask is-pending" aria-label={session.title ?? session.prompt}>
      {session.title !== undefined && <div className="lc-cloudtask__title">{session.title}</div>}
      <div className="lc-cloudtask__prompt">{session.prompt}</div>
      <div className="lc-cloudtask__state">
        <span>
          {known
            ? `In Claude’s cloud since ${agoLabel(session.startedAt) ?? 'just now'}.`
            : `Sent to Claude’s cloud ${agoLabel(session.startedAt) ?? 'just now'}. Follow it on claude.ai or in the Claude app.`}
        </span>
      </div>
      {session.note !== undefined && <p className="lc-cloudtask__note">{session.note}</p>}
      <div className="lc-cloudtask__actions">
        {known && (
          <>
          <button type="button" className="lc-primarybutton" disabled={checking} onClick={check} title="Claude Code brings the session in out of sight, in a worktree of its own; this folder is not touched">
            {checking ? 'Reading it…' : read === undefined ? 'Show what it did' : 'Check again'}
          </button>
          <button type="button" className="lc-button" aria-pressed={watching} disabled={applying} onClick={() => { setExpanded(true); setWatching((on) => !on) }} title="Read every 90 seconds; stops after two unchanged reads, 30 minutes, or when you hide this row">
            {watching ? 'Stop watching' : 'Watch'}
          </button>
          {expanded && <button type="button" className="lc-button" onClick={() => { setExpanded(false); setWatching(false) }}>Hide what it did</button>}
          </>
        )}
        <button type="button" className={known ? 'lc-button' : 'lc-primarybutton'} onClick={() => onOpenWeb(session.url)} title={known ? 'This session, on claude.ai' : 'Your Claude Code sessions, in your browser'}>
          See it on claude.ai
        </button>
        {known && <button type="button" className="lc-button" disabled={openingTerminal || checking} onClick={() => {
          setWatching(false)
          setOpeningTerminal(true)
          void onContinue(session.id).then((problem) => {
            setOpeningTerminal(false)
            setSaid(problem ?? 'Claude Code opened its own copy in a terminal. Locust does not see what happens there; use /remote-control in Claude Code to steer from your phone.')
          }).catch(() => { setOpeningTerminal(false); setSaid('The terminal could not be opened. Open this session on claude.ai.') })
        }}>{openingTerminal ? 'Opening terminal…' : 'Continue in terminal'}</button>}
        <button
          type="button"
          className="lc-button"
          onClick={() => onHome(session.id)}
          title={known ? 'Claude Code brings this session’s work into this folder' : 'Opens Claude Code here with its list of cloud sessions: pick this one and its work comes into this folder'}
        >
          Bring it home
        </button>
        <button type="button" className="lc-ghostbutton lc-iconbutton" aria-label={`Remove "${session.prompt}" from this list`} title="Remove from this list (the session itself stays on claude.ai)" onClick={() => onForget(session.id)}>
          <Icon name="close" size={13} />
        </button>
      </div>
      {known && <p className="lc-cloudtask__note">Continue in terminal opens its own copy in a fresh worktree. Locust does not see what happens there. Use /remote-control in Claude Code to keep steering from your phone.</p>}
      {expanded && reading?.ok === false && <p className="lc-cloudtask__note" role="alert">{reading.message}</p>}
      {expanded && read !== undefined && (
        <div className="lc-cloudtask__read">
          <p className="lc-cloudtask__note" role="status" data-cloud-checked-at={read.checkedAt}>{cloudUpdatedLabel(read.checkedAt, clock)}</p>
          {(read.exchanges.length > 0 || read.note === undefined) && (
            <p className="lc-cloudtask__note">
              {read.exchanges.length === 0
                ? 'Claude Code brought it in, but it has no finished reply yet.'
                : `What it said, as of ${agoLabel(read.checkedAt) ?? 'just now'}:`}
            </p>
          )}
          {read.exchanges.length > 0 && (
            <ol className="lc-cloudtask__thread" aria-label="The session’s conversation">
              {read.exchanges.map((exchange, index) => (
                <li key={`${exchange.at}:${String(index)}`} className="lc-cloudtask__exchange">
                  <div className="lc-cloudtask__ask">{exchange.prompt}</div>
                  {exchange.answer !== undefined && (
                    <div className="lc-cloudtask__reply">
                      <AgentText text={exchange.answer} streaming={false} />
                    </div>
                  )}
                </li>
              ))}
            </ol>
          )}
          {read.note !== undefined && <p className="lc-cloudtask__note">{read.note}</p>}
          {read.changeTooBig === true && <p className="lc-cloudtask__note">Its change is too big to show here. Apply still brings it in whole.</p>}
          {(files.length > 0 || read.changeTooBig === true) && (
            <>
              {files.length > 0 && (
                <div className="lc-review__files">
                  {files.map((file) => {
                    const counts = fileCounts(file)
                    return (
                      <section key={`${session.id}:${file.path}`} aria-label={file.path}>
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
              <div className="lc-cloudtask__actions">
                <button type="button" className="lc-primarybutton" disabled={applying || applied} onClick={apply} title="Its change comes into this folder, not committed, so you can look before you keep it">
                  {applied ? 'Applied' : applying ? 'Applying…' : 'Apply to this folder'}
                </button>
              </div>
            </>
          )}
          {read.diff === undefined && read.changeTooBig !== true && read.note === undefined && (
            <p className="lc-cloudtask__note">It has changed nothing yet.</p>
          )}
        </div>
      )}
      {known && (
        <form
          className="lc-cloudtask__more"
          onSubmit={(event) => {
            event.preventDefault()
            send()
          }}
        >
          <input
            className="lc-input"
            value={draft}
            placeholder="Tell it more…"
            aria-label={`Tell "${session.title ?? session.prompt}" more`}
            disabled={sending}
            onChange={(event) => setDraft(event.target.value)}
          />
          <button type="submit" className="lc-button" disabled={sending || draft.trim().length === 0}>
            {sending ? 'Sending…' : 'Send'}
          </button>
        </form>
      )}
      {said !== undefined && <p className="lc-cloudtask__note" role="status">{said}</p>}
    </section>
  )
}
