import { createContext } from 'react'

import type { StatusTone } from './statusChips.js'

/*
 * A CALL'S TO-DOS, AND HOW EACH IS GOING (0.755). Colin passed it on 2026-10-12 ("I'll run all your suggestions"),
 * from Grok Bot's voice mode, where a summary arrives after the call: everything a Talk call handed out, to whom, and
 * where it stands now -- one line each, read from the same run and record the sidebar reads, so a to-do says Done
 * when its teammate has finished and Needs you while it waits on the person.
 */
export type CallTodoState = 'starting' | 'working' | 'needs-you' | 'done' | 'failed' | 'stopped'

export const CALL_TODO_WORDS: Readonly<Record<CallTodoState, { readonly label: string; readonly tone: StatusTone }>> = {
  starting: { label: 'Starting', tone: 'quiet' },
  working: { label: 'Working', tone: 'blue' },
  'needs-you': { label: 'Needs you', tone: 'amber' },
  done: { label: 'Done', tone: 'green' },
  failed: { label: 'Failed', tone: 'red' },
  stopped: { label: 'Stopped', tone: 'quiet' }
}

/**
 * One to-do's state: the run while the window holds it, else the record. Waiting on the person outranks both -- an
 * approval mid-run, or a question a finished turn stopped on. A to-do the window has heard of but not yet seen run
 * is starting.
 */
export function callTodoState(facts: { readonly runPhase?: string; readonly recordPhase?: string; readonly needsYou: boolean }): CallTodoState {
  if (facts.needsYou) return 'needs-you'
  const phase = facts.runPhase ?? facts.recordPhase
  if (phase === 'completed') return 'done'
  if (phase === 'failed') return 'failed'
  if (phase === 'cancelled' || phase === 'interrupted') return 'stopped'
  if (phase === 'running' || phase === 'cancelling') return 'working'
  return 'starting'
}

/** What the window knows about the to-dos on screen: each one's state, and how to open its conversation. */
export interface CallTodos {
  readonly states: ReadonlyMap<string, CallTodoState>
  readonly open: (missionId: string) => void
  /** A posted peer message's receiving turn, only once the window has seen its delivery. */
  readonly receivedIn: ReadonlyMap<string, string>
}

export const CallTodosContext = createContext<CallTodos>({ states: new Map(), open: () => undefined, receivedIn: new Map() })

/** A call or typed turn's to-do. An undelivered share has a key, but no receiving mission yet. */
export interface TodoShown {
  readonly key?: string
  readonly missionId?: string
  readonly name: string
  readonly task: string
}

/** "3 to-dos", "1 to-do". */
export function todoCount(count: number): string {
  return `${String(count)} ${count === 1 ? 'to-do' : 'to-dos'}`
}
