import type { ReactElement } from 'react'

import type { MissionMode, TeammateRole } from '../../../shared/ipc.js'
import { Icon } from './Icon.js'
import type { IconName } from './Icon.js'

/**
 * What a conversation can begin with, on Home (2026-10-19). Colin, of Claude Code's own starters beside the "Try two
 * models on" chips: "maybe with a compare button or dropdown beside it? so the user can decide whether to start it
 * with a teammate or compare different models". A chip held a noun; a row says what it does. Pressing the row puts
 * the words in the box for the teammate on screen, in the mode they suit; Compare puts the same words in a
 * comparison of two models. Neither sends: Send is the next press.
 */
export interface Starter {
  readonly id: string
  readonly label: string
  readonly line: string
  readonly icon: IconName | 'page'
  /** The words the box is filled with. One ending in ": " is finished by the person. */
  readonly prompt: string
  /** The mode the row sets for a teammate's turn; none keeps the mode on screen. */
  readonly mode?: MissionMode
  /** A comparison of the WORK (each model changes its own copy) rather than of answers. */
  readonly compareChanges: boolean
}

export const STARTERS: readonly Starter[] = [
  { id: 'plan', label: 'Plan before building', line: 'Agree on the approach before anything changes', icon: 'route', mode: 'plan', compareChanges: false, prompt: 'Read the code first, change nothing, and plan how to: ' },
  { id: 'debug', label: 'Debug an issue', line: 'Find the root cause and fix it', icon: 'target', compareChanges: true, prompt: 'Find the root cause of this bug, fix it, and show what you changed: ' },
  { id: 'review', label: 'Review my changes', line: 'Read this branch’s changes like a careful reviewer', icon: 'eye', mode: 'ask', compareChanges: false, prompt: 'Review the changes on this branch as a careful reviewer. List bugs and risks by severity, each with its file and line. Change nothing.' },
  { id: 'page', label: 'Build a landing page', line: 'For whatever you’re launching, built and running', icon: 'page', compareChanges: true, prompt: 'Make index.html: a one-page landing page with a headline, what it offers, and a clear way to get in touch. One file, all CSS inside it. It is for: ' }
]

/**
 * HOME'S STARTERS FOR A TEAM WITH NO CODER (2026-11-01). The four rows above are a coder's work. A new person who pressed
 * Research & money was offered "Debug an issue" and "Build a landing page" with nobody on the team to do them (the
 * first-session drive on 0.845). A team without a coder gets a row for each teammate's own work instead, by the role
 * on their card (its title for a custom one), and a row to plan first; each goes to the teammate whose work it is.
 */
const ROLE_STARTERS: Readonly<Record<string, Omit<Starter, 'id'>>> = {
  'Research & Briefs': { label: 'Research a question', line: 'What the sources say, then a conclusion', icon: 'search', compareChanges: false, prompt: 'Research this and write a short brief: what the sources say, each with its date, then your own conclusion, kept apart: ' },
  'Data & Reporting': { label: 'Work out the numbers', line: 'Every step shown, the result in a table', icon: 'grid', compareChanges: false, prompt: 'Work this out with every step of the arithmetic shown, and put the result in a table: ' },
  'Chief of Staff': { label: 'Ask the whole team', line: 'Each teammate’s part, brought back as one answer', icon: 'users', compareChanges: false, prompt: 'Ask the team about this: get each teammate’s part, then bring me one answer: ' },
  'Ops & Scheduling': { label: 'Put it on a schedule', line: 'What comes first, and when each part is done', icon: 'clock', compareChanges: false, prompt: 'Put this on a schedule: what comes first, what can wait, and when each part will be done: ' },
  'Docs & QA': { label: 'Check a document', line: 'What is wrong, unclear or missing', icon: 'eye', compareChanges: false, prompt: 'Check this carefully and list what is wrong, unclear or missing, most important first: ' },
  Writer: { label: 'Write a first draft', line: 'One page, in the voice you ask for', icon: 'pencil', compareChanges: false, prompt: 'Write a first draft of this, one page, and say which voice you chose: ' },
  Designer: { label: 'Make a design', line: 'Colours, fonts and a layout, as a file', icon: 'palette', compareChanges: false, prompt: 'Design this as a file in this folder: a colour palette with hex codes, two font pairings and a layout: ' },
  Editor: { label: 'Edit for clarity', line: 'What is unclear, wrong or too long', icon: 'check', compareChanges: false, prompt: 'Edit this for clarity: mark what is unclear, wrong or too long, then give me the edited version: ' }
}
/** "Plan before building", for work that is not code: it fills the row a third teammate leaves in a grid of two. */
const PLAN_MAKING: Starter = { id: 'plan-make', label: 'Plan before making', line: 'Agree on the approach before anything is made', icon: 'route', mode: 'plan', compareChanges: false, prompt: 'Change nothing yet, and plan how to: ' }

/**
 * The rows Home offers a team: the coder's four while anyone on it codes (or nobody is on it yet); otherwise one for each
 * teammate's own work, in the team's order, up to three, and Plan before making. A team whose roles say nothing that fits
 * keeps the four.
 */
export function startersForTeam(team: readonly { readonly role: string }[]): readonly Starter[] {
  if (team.length === 0 || team.some((mate) => mate.role === 'Code & Migrations')) return STARTERS
  const rows: Starter[] = []
  for (const mate of team) {
    const starter = ROLE_STARTERS[mate.role]
    if (starter === undefined || rows.some((row) => row.id === `role:${mate.role}`)) continue
    rows.push({ id: `role:${mate.role}`, ...starter })
    if (rows.length === STARTERS.length - 1) break
  }
  return rows.length === 0 ? STARTERS : [...rows, PLAN_MAKING]
}

/**
 * WHO ON THE TEAM A HOME STARTER SUITS (2026-10-30). On Home no teammate is on screen, so a row's words went to the agent
 * in the chat box by itself -- right after a new person had pressed Build software and made Wren, Juno and Atlas. With a
 * team, the row goes to the teammate whose work it is: the coder for planning, debugging and a page, the reviewer for a
 * review (the coder when there is none). A role's own row goes to that role, and Plan before making to the chief of
 * staff, else the first teammate (2026-11-01). `role` is the card's role, its title for a custom one (roleLabelOf).
 * Undefined when nobody's role fits, and the words wait in the box as before.
 */
export function teammateForStarter(starterId: string, team: readonly { readonly teammateId: string; readonly role: string }[]): string | undefined {
  if (starterId.startsWith('role:')) return team.find((one) => one.role === starterId.slice('role:'.length))?.teammateId
  if (starterId === PLAN_MAKING.id) return (team.find((one) => one.role === 'Chief of Staff') ?? team[0])?.teammateId
  const wanted: readonly TeammateRole[] = starterId === 'review' ? ['Docs & QA', 'Code & Migrations'] : ['Code & Migrations']
  for (const role of wanted) {
    const mate = team.find((one) => one.role === role)
    if (mate !== undefined) return mate.teammateId
  }
  return undefined
}

/** The landing page drawn as itself (the 0.461 drawing): a browser window with a headline. */
function PageIcon(): ReactElement {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="3" y="4" width="18" height="16" rx="2.5" />
      <path d="M3 8.5h18" />
      <path d="M7 12.5h10" />
      <path d="M7 15.5h6" />
    </svg>
  )
}

export function StarterList({ starters = STARTERS, onStart, onCompareStart, onCompare, onIssue }: {
  /** The rows to offer: startersForTeam's for the team on Home. */
  readonly starters?: readonly Starter[]
  readonly onStart: (starter: Starter) => void
  readonly onCompareStart: (starter: Starter) => void
  /** Compare on words of the person's own, from an empty box. */
  readonly onCompare?: () => void
  /** The folder's open GitHub issues. Absent when this folder is not one gh can read. */
  readonly onIssue?: () => void
}): ReactElement {
  return (
    <section className="lc-homestarts" aria-label="Start a conversation">
      {/* One line, as the accounts' "Show all" sits on theirs: Compare on words of your own, at the heading's end. */}
      <div className="lc-homestarts__head">
        <span className="lc-agenthead__label">Start a conversation</span>
        {/*
          * FROM AN ISSUE, IN THE HEAD (0.799): a fifth starter was a third row of two, and Home scrolled again at
          * 1205x803 (Colin, 2026-10-19: "fit all this in one screen with no scrolling"). Beside Compare models,
          * the other way in that is not one of the four.
          */}
        {onIssue !== undefined && (
          <button type="button" className="lc-buildhead__compare lc-homestarts__issue" title="An open GitHub issue in this folder, with its text and pictures. Choosing one fills the box; nothing is sent until you press Send." onClick={onIssue}>
            <Icon name="inbox" size={12} />
            Start from an issue
          </button>
        )}
        {onCompare !== undefined && (
          // "Compare models", word for word: six drives and the composer say it so (0.610).
          <button type="button" className="lc-buildhead__compare lc-homestarts__anything" title="Ask two or three models anything and see the answers side by side" onClick={onCompare}>
            Compare models
            <Icon name="chevron-right" size={12} />
          </button>
        )}
      </div>
      <ul className="lc-homestarts__list">
        {starters.map((starter) => (
          <li key={starter.id} className="lc-homestart">
            <button type="button" className="lc-homestart__start" title={`${starter.line}. Puts the words in the box for your teammate${starter.mode === undefined ? '' : `, in ${starter.mode === 'plan' ? 'Plan' : 'Ask'} mode`}. Nothing is sent until you press Send.`} onClick={() => onStart(starter)}>
              <span className="lc-homestart__icon" aria-hidden="true">{starter.icon === 'page' ? <PageIcon /> : <Icon name={starter.icon} size={16} />}</span>
              <span className="lc-homestart__name">{starter.label}</span>
              <span className="lc-homestart__line">{starter.line}</span>
            </button>
            <button
              type="button"
              className="lc-homestart__compare"
              aria-label={`Compare models on: ${starter.label}`}
              title={starter.compareChanges ? 'Two models do this side by side, each in its own copy; you keep the one you like' : 'Two models answer this side by side'}
              onClick={() => onCompareStart(starter)}
            >
              <Icon name="columns" size={13} />
              Compare
            </button>
          </li>
        ))}
      </ul>
    </section>
  )
}
