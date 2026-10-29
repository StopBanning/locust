import type { MissionApprovalRequest } from '../shared/ipc.js'

/**
 * Getting a person's attention when the window does not have it.
 *
 * An approval stops a run until it is answered. When the window is in front
 * the amber card is enough; when it is behind another app, or minimized, the
 * run sits waiting and nobody knows. So an approval that arrives while the
 * window is not focused becomes an OS notification, and clicking it brings
 * the window forward. Nothing else is notified: a notification for every
 * finished run would train a person to dismiss the one that matters.
 *
 * The decision is pure so it can be tested; the Electron calls are injected.
 */

export interface AttentionSurface {
  /** Whether the window currently has the person's attention. */
  readonly focused: () => boolean
  /** Whether the OS offers notifications at all. */
  readonly supported: () => boolean
  /** Show one; `onClick` runs when it is clicked. */
  readonly notify: (input: { readonly title: string; readonly body: string; readonly onClick: () => void }) => void
  /** Bring the window forward. */
  readonly focusWindow: () => void
  /** Open a conversation in the window (0.379: a finished run's toast opens where it finished). */
  readonly openMission?: (missionId: string) => void
}

/**
 * How long finishes are gathered before one toast says them (0.379): two
 * teammates ending within a few seconds of each other are one thing to hear.
 */
export const FINISH_TOAST_WINDOW_MS = 4_000

/** What the renderer asks to be said about a finished run, bounded -- it is a toast, not a transcript. */
export function finishFrom(raw: unknown): { readonly title: string; readonly body: string; readonly missionId: string | undefined } | undefined {
  if (typeof raw !== 'object' || raw === null) return undefined
  const value = raw as { title?: unknown; body?: unknown; missionId?: unknown }
  if (typeof value.title !== 'string' || typeof value.body !== 'string') return undefined
  const title = value.title.replace(/\s+/g, ' ').trim().slice(0, 120)
  const body = value.body.trim().slice(0, 400)
  if (title.length === 0) return undefined
  return { title, body, missionId: typeof value.missionId === 'string' && value.missionId.length > 0 && value.missionId.length <= 200 ? value.missionId : undefined }
}

/** Several finishes, as one toast: each said in a line, the newest last, three at most. */
export function finishedNotificationText(finishes: readonly { readonly title: string; readonly body: string }[]): { readonly title: string; readonly body: string } {
  if (finishes.length === 1) return { title: finishes[0]!.title, body: finishes[0]!.body }
  const shown = finishes.slice(-3)
  const hidden = finishes.length - shown.length
  return {
    title: `${String(finishes.length)} teammates are done`,
    body: [...(hidden > 0 ? [`… and ${String(hidden)} more`] : []), ...shown.map((finish) => `${finish.title}: ${finish.body}`)].join('\n')
  }
}

export function shouldNotify(input: { readonly focused: boolean; readonly supported: boolean }): boolean {
  return input.supported && !input.focused
}

/** The words on the notification. Never the exact command: a toast is not a place for a secret. */
export function approvalNotificationText(
  request: Pick<MissionApprovalRequest, 'kind' | 'summary'>,
  teammateName: string | undefined
): { readonly title: string; readonly body: string } {
  const who = teammateName ?? 'A teammate'
  const title = request.kind === 'question' ? `${who} has a question` : `${who} needs your approval`
  return { title, body: `${request.summary} · the run is paused until you answer.` }
}

/**
 * How long a room's changes are gathered before one toast says them all.
 *
 * The spec's bar (docs/FEATURES-FROM-VISION-2026-09-05.md, #2): a room of
 * three working unattended for ten minutes must produce a readable set of
 * toasts, not a storm. Three teammates each moving the board at the end of
 * a run can land inside the same few seconds; one toast per change would be
 * three toasts saying one thing. So changes to a room are held for this
 * long and said once, newest last, three lines at most.
 *
 * MEASURED 2026-09-05 with the test below: at twenty seconds, three
 * teammates ending runs thirty seconds apart produced twenty toasts in ten
 * minutes -- two a minute, which is the storm. Two minutes gathers the
 * same ten minutes into five, each saying three or four changes, and a
 * person who has stepped away is not worse off for hearing about a board
 * two minutes late. The window that is in front hears nothing either way.
 */
/** A teammate asked and stopped: the question, bounded to a toast. */
export function decisionNotificationText(teammateName: string | undefined, question: string): { readonly title: string; readonly body: string } {
  const one = question.replace(/\s+/g, ' ').trim()
  return {
    title: `${teammateName ?? 'A teammate'} is asking you something`,
    body: `${one.length > 140 ? `${one.slice(0, 139).trimEnd()}…` : one} · the run waits for your answer.`
  }
}

/** A run stopped at its account's limit: where it stopped, in the runtime's own words when it had any. */
export function limitNotificationText(
  teammateName: string | undefined,
  runtimeName: string,
  message: string | undefined
): { readonly title: string; readonly body: string } {
  const said = message === undefined ? undefined : message.replace(/\s+/g, ' ').trim()
  return {
    title: `${teammateName ?? 'A teammate'} hit the ${runtimeName} limit`,
    body: `${said === undefined || said.length === 0 ? 'The run stopped at a checkpoint' : said.length > 120 ? `${said.slice(0, 119).trimEnd()}…` : said} · pick where it continues.`
  }
}

export const ROOM_TOAST_WINDOW_MS = 120_000
export const ROOM_TOAST_LINES = 3

export interface AttentionTimers {
  readonly schedule: (task: () => void, ms: number) => unknown
  readonly clear: (handle: unknown) => void
}

export function roomNotificationText(
  roomName: string,
  messages: readonly string[]
): { readonly title: string; readonly body: string } {
  const shown = messages.slice(-ROOM_TOAST_LINES)
  const hidden = messages.length - shown.length
  return {
    title: messages.length === 1 ? roomName : `${roomName} · ${String(messages.length)} changes`,
    body: (hidden > 0 ? [`… and ${String(hidden)} more`, ...shown] : shown).join('\n')
  }
}

export function createAttention(
  surface: AttentionSurface,
  timers: AttentionTimers = { schedule: (task, ms) => setTimeout(task, ms), clear: (handle) => clearTimeout(handle as NodeJS.Timeout) }
): {
  approvalArrived(request: Pick<MissionApprovalRequest, 'kind' | 'summary'>, teammateName: string | undefined): boolean
  /**
   * A room's board or thread moved. Gathered per room and said once per
   * window; nothing while the window has the person's attention, because
   * the room screen already says it.
   */
  roomChanged(input: { readonly roomId: string; readonly roomName: string; readonly message: string }): void
  /** A teammate ended a run with a question card. Said at once, like an approval. */
  decisionAsked(teammateName: string | undefined, question: string): boolean
  /** An app asks to use the team from anywhere (0.841, remote-mcp.ts): its card waits in the window. Said at once. */
  connectAsked(appName: string): boolean
  /** A run stopped at its account limit and waits for a person to pick where it continues. */
  limitHit(teammateName: string | undefined, runtimeName: string, message: string | undefined): boolean
  /**
   * A run the person started finished, after long enough to have walked away
   * (the renderer decides that: finishedToast.ts). Gathered briefly, said once,
   * and only while the window is elsewhere; a click opens the conversation.
   */
  runFinished(finish: { readonly title: string; readonly body: string; readonly missionId: string | undefined }): void
  /** Test seam: what is waiting to be said. */
  pending(): ReadonlyMap<string, readonly string[]>
} {
  const held = new Map<string, { readonly roomName: string; readonly messages: string[]; readonly handle: unknown }>()
  let finishes: { readonly title: string; readonly body: string; readonly missionId: string | undefined }[] = []
  const flushFinishes = (): void => {
    const said = finishes
    finishes = []
    if (said.length === 0) return
    if (!shouldNotify({ focused: surface.focused(), supported: surface.supported() })) return
    // The newest finish with a conversation is the one a click opens.
    const opens = [...said].reverse().find((finish) => finish.missionId !== undefined)?.missionId
    surface.notify({
      ...finishedNotificationText(said),
      onClick: () => {
        surface.focusWindow()
        if (opens !== undefined) surface.openMission?.(opens)
      }
    })
  }
  const flush = (roomId: string): void => {
    const entry = held.get(roomId)
    held.delete(roomId)
    if (entry === undefined || entry.messages.length === 0) return
    if (!shouldNotify({ focused: surface.focused(), supported: surface.supported() })) return
    surface.notify({ ...roomNotificationText(entry.roomName, entry.messages), onClick: () => surface.focusWindow() })
  }
  return {
    approvalArrived(request, teammateName) {
      if (!shouldNotify({ focused: surface.focused(), supported: surface.supported() })) return false
      const text = approvalNotificationText(request, teammateName)
      surface.notify({ ...text, onClick: () => surface.focusWindow() })
      return true
    },
    decisionAsked(teammateName, question) {
      if (!shouldNotify({ focused: surface.focused(), supported: surface.supported() })) return false
      surface.notify({ ...decisionNotificationText(teammateName, question), onClick: () => surface.focusWindow() })
      return true
    },
    connectAsked(appName) {
      if (!shouldNotify({ focused: surface.focused(), supported: surface.supported() })) return false
      surface.notify({ title: `${appName.slice(0, 60)} wants to use your Locust team`, body: 'Allow or deny it in Locust’s window. Nothing is connected until you do.', onClick: () => surface.focusWindow() })
      return true
    },
    limitHit(teammateName, runtimeName, message) {
      if (!shouldNotify({ focused: surface.focused(), supported: surface.supported() })) return false
      surface.notify({ ...limitNotificationText(teammateName, runtimeName, message), onClick: () => surface.focusWindow() })
      return true
    },
    runFinished(finish) {
      finishes.push(finish)
      if (finishes.length === 1) timers.schedule(flushFinishes, FINISH_TOAST_WINDOW_MS)
    },
    roomChanged(input) {
      const entry = held.get(input.roomId)
      if (entry !== undefined) {
        entry.messages.push(input.message)
        return
      }
      const handle = timers.schedule(() => flush(input.roomId), ROOM_TOAST_WINDOW_MS)
      held.set(input.roomId, { roomName: input.roomName, messages: [input.message], handle })
    },
    pending() {
      return new Map([...held.entries()].map(([roomId, entry]) => [roomId, [...entry.messages]]))
    }
  }
}
