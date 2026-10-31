import type { ReactElement } from 'react'

import type { MissionMode, PublicTeammate, TeammateRole } from '../../../shared/ipc.js'
import { ROLE_DESCRIPTIONS, roleLabelOf } from '../../../shared/ipc.js'
import { freshStartersFor } from '../../../shared/team-templates.js'
import { modeSentence } from '../status.js'
import { TeammateBot } from './TeammateBot.js'

/**
 * What the current mode lets this teammate do, said for THAT mode. The
 * sentence used to be fixed -- "Nothing is changed unless you pick a mode
 * that allows it" -- above a composer whose default is Accept edits, so a
 * fresh Research teammate promised read-only while the footer said it may
 * edit the workspace (user session 1, 2026-09-05).
 */
/*
 * Moved to `status.ts` beside every other phrasing of a mode. Re-exported
 * because this is where callers already look for it.
 */
export { modeSentence }

/**
 * The idle teammate: a capability-led empty state.
 *
 * The design's rule for this surface is to lead with what the teammate is good
 * at rather than with an empty box. The starters below come from the ROLE the
 * user chose, so they describe work this teammate was actually set up for --
 * and each is a prompt that can be sent as-is, not a category heading.
 *
 * They are suggestions, not capabilities: everything here is read-only phrasing
 * that any runtime can attempt, so a starter cannot promise something the
 * sandbox would refuse.
 *
 * A teammate made from a team template brings its own (0.359), in its team's
 * world; these are for a teammate made by hand. Only the roles that ARE code
 * speak of code: a Research or Data teammate may be looking at a folder of
 * statements or drafts, and "this codebase" told a person who picked
 * Research & money that Locust was not for them.
 */
const STARTERS: Readonly<Record<TeammateRole, readonly string[]>> = {
  'Code & Migrations': [
    'Read this project and tell me what it does, in one paragraph.',
    'Find the riskiest file in this repo and explain why.',
    'What would break first if traffic doubled?'
  ],
  'Research & Briefs': [
    'Summarize what is in this folder, for someone seeing it for the first time.',
    'List the decisions already made here that would be expensive to reverse.',
    'What would you want to know before you started on anything here?'
  ],
  'Ops & Scheduling': [
    'What work here repeats, and which of it could run on a schedule?',
    'Which failures here would nobody notice for a week?',
    'Make me a checklist for the routine work this folder seems to need.'
  ],
  'Docs & QA': [
    'Find anything written here that no longer matches what it describes.',
    'What here is unchecked that would hurt most if it were wrong?',
    'Read the main document here and tell me what a new reader would still get wrong.'
  ],
  'Data & Reporting': [
    'Find the numbers in this folder and say where each one comes from.',
    'What would a weekly summary of the work here contain?',
    'Which figure here would you check first, and why?'
  ],
  'Chief of Staff': [
    'Look at the team and tell me who should take what.',
    'Ask each teammate for one thing they would do first, and bring me the list.',
    'What is the state of the work here? Delegate the reading and report back.'
  ],
  Custom: [
    'What can you help me with? Give me three concrete examples.',
    'Ask me three questions about what I need, then suggest where to start.',
    'Look at the files in this folder and tell me what you could do with them.'
  ]
}

/**
 * WHILE THE FOLDER HOLDS NOTHING YET (2026-10-30): starters that make something, for a teammate made by hand. A new
 * person's first folder is the empty one Locust made, and every starter above asks about files that are not there.
 * A template teammate brings its own (team-templates.ts freshStarters).
 */
const FRESH_STARTERS: Readonly<Record<TeammateRole, readonly string[]>> = {
  'Code & Migrations': [
    'Make a small web page here that shows today\'s date and a quote, and tell me how to open it.',
    'Ask me what I want to build, then set up its first files here.',
    'Write a small script here that renames photos by the date they were taken, and explain how to run it.'
  ],
  'Research & Briefs': [
    'Ask me what I am researching, then write a one-page brief on it, with sources.',
    'Compare three ways to make a simple website today: what each costs and how hard it is, with sources.',
    'What should I know before I start a project like mine? Ask me two questions first.'
  ],
  'Ops & Scheduling': [
    'Ask me what I do every week, then say which of it could run on a schedule.',
    'Write a weekly plan I can fill in each Monday, as a file here.',
    'Make me a checklist for the routine work of a small business, as a file here.'
  ],
  'Docs & QA': [
    'Write me a checklist for testing a website before I share it.',
    'Ask me about a project of mine, then write the README it should have.',
    'Explain what a careful review looks for, with one short example of each point.'
  ],
  'Data & Reporting': [
    'Make me a monthly budget spreadsheet here, with categories, totals and a line for savings.',
    'Make a spreadsheet here to track hours worked, with a weekly total.',
    'Ask me what I want to keep track of, then set up a spreadsheet for it here.'
  ],
  'Chief of Staff': [
    'Look at the team and tell me who should take what.',
    'Ask each teammate for one thing they would do first, and bring me the list.',
    'Ask me what I am working on, then split it between the team.'
  ],
  Custom: [
    'What can you help me with? Give me three concrete examples.',
    'Ask me three questions about what I need, then suggest where to start.',
    'Make something small here that shows what you can do, and tell me how to open it.'
  ]
}

/** The starters to offer: a template teammate's own, or its role's; while the folder is empty, the ones that make something. */
export function startersFor(teammate: Pick<PublicTeammate, 'role' | 'starters'>, folderEmpty: boolean): readonly string[] {
  if (folderEmpty) {
    if (teammate.starters === undefined) return FRESH_STARTERS[teammate.role] ?? FRESH_STARTERS.Custom
    const fresh = freshStartersFor(teammate.starters)
    if (fresh !== undefined) return fresh
  }
  return teammate.starters ?? STARTERS[teammate.role] ?? STARTERS.Custom
}

export function IdleTeammate({
  teammate,
  canStart,
  blocked,
  onStarter,
  mode,
  folderEmpty = false
}: {
  readonly teammate: PublicTeammate
  readonly canStart: boolean
  /**
   * Why this teammate cannot start at all, when it cannot -- the runtime is
   * installed and signed out. Absent when it can.
   *
   * Separate from `canStart`, which only means a mission is already running.
   * Conflating them is what let this screen offer starter buttons to a
   * teammate whose runtime was signed out (Astra's Finding 1, 2026-09-14).
   */
  readonly blocked?: string
  readonly onStarter: (prompt: string) => void
  /** The composer's current permission mode: the sentence below must say what THIS mode does. */
  readonly mode: MissionMode
  /** The folder holds nothing yet but Locust's own (dot) files: the starters make something rather than read it. */
  readonly folderEmpty?: boolean
}): ReactElement {
  const starters = startersFor(teammate, folderEmpty)
  // A Custom teammate's title is what they do; a built-in role says it in a few words.
  const about = teammate.role === 'Custom' ? undefined : ROLE_DESCRIPTIONS[teammate.role]

  return (
    <div className="lc-empty">
      <div className="lc-empty__inner">
        {/* The face you are about to talk to: it listens as you type, and is itself meanwhile (faceLife.ts). */}
        <TeammateBot hue={teammate.hue} avatar={teammate.avatar} size={56} teammateId={teammate.teammateId} hears />
        <h1>{teammate.name}</h1>
        <p>
          {roleLabelOf(teammate)}
          {about === undefined ? '' : ` · ${about}`}. {modeSentence(mode)}
        </p>

        <div className="lc-starters">
          {starters.map((prompt) => (
            <button
              key={prompt}
              type="button"
              className="lc-starter"
              disabled={!canStart || blocked !== undefined}
              onClick={() => onStarter(prompt)}
            >
              {prompt}
            </button>
          ))}
        </div>

        <p className={`lc-footnote${blocked === undefined ? '' : ' lc-tone-red'}`}>
          {blocked !== undefined
            ? blocked
            : canStart
              ? 'Pick one, or say what you need below.'
              : 'Connect a runtime to start a mission.'}
        </p>
      </div>
    </div>
  )
}
