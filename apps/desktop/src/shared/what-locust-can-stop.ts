import type { MissionRuntimeId } from '@teammate/runtime-adapters'

import { runtimeDisplayName } from './runtimes.js'

/**
 * WHAT LOCUST CAN STOP, AI agent by AI agent (0.617; the PRD's R9 and A1).
 *
 * Locust does not run an agent's tools; each AI agent runs its own, and
 * Locust can stop one only where that agent asks first. Where it asks, a card
 * waits for the person, their saved rules answer before it reaches them, and
 * the record says who decided (shared/who-decides.ts). Where it does not ask,
 * the mode decides what the agent is given, and nothing is stopped one action
 * at a time. A buyer reading the public copy, and a person choosing a mode,
 * should both be told which is which -- in the same words.
 *
 * So this is the one source: Settings > AI agents draws it, and
 * docs/WHAT-LOCUST-CAN-STOP.md is written from it (the test fails when the
 * file and this disagree). Each line was read off the code that builds the
 * run -- packages/runtime-adapters/src/commands.ts, the approval paths in
 * main/ -- and the modes off RUNTIME_CAPABILITIES; the test holds the reach
 * of each to those facts. Plan runs wherever Ask does, and reads like it.
 * Gemini is left out: no run can start on it.
 */

/** How much of a run Locust's card can stop, at most. */
export type StopReach = 'each-action' | 'some-actions' | 'questions-only' | 'nothing'

export interface StopRow {
  readonly runtime: MissionRuntimeId
  readonly reach: StopReach
  /** What it asks Locust before doing. */
  readonly asks: string
  /** What it does without asking, mode by mode. */
  readonly without: string
  /** Who keeps an "Always", where there is one. */
  readonly always?: string
  /** The table's "Asks first" cell, where the reach's own words would not be true of this agent. */
  readonly asksBrief?: string
}

export const STOP_REACH_LABEL: Readonly<Record<StopReach, string>> = {
  'each-action': 'Each action',
  'some-actions': 'Some actions',
  'questions-only': 'Questions only',
  nothing: 'Nothing'
}

const KEPT_BY_LOCUST = 'Locust keeps an Always for the rest of the run, and your saved rules come first.'

export const WHAT_LOCUST_CAN_STOP: readonly StopRow[] = [
  {
    runtime: 'codex',
    reach: 'each-action',
    asks: 'In Approve each action: every command that does more than read, and every file change, before it runs.',
    without: 'In Ask and Plan it works in Codex’s own sandbox, where its commands can read but change nothing; in Edit, where they can also change files in this folder. It asks nothing in those modes, or in Auto.',
    always: KEPT_BY_LOCUST
  },
  {
    runtime: 'claude',
    reach: 'some-actions',
    asks: 'In Ask and Plan, every connector call. In Edit, a connector call the teammate was not given (or every one, when “Ask before every connector call” is on), a command it does not run on its own say, and a change to a file outside this folder.',
    without: 'Reading files, in every mode. In Edit, changes to files in this folder and the commands Claude Code runs on its own say. In Auto, everything. In every mode, Locust refuses one kind of command without asking: one that ends a browser, Locust or another AI agent that no teammate started, by name or by its id, which would end yours too.',
    always: KEPT_BY_LOCUST
  },
  {
    runtime: 'opencode',
    reach: 'each-action',
    asks: 'In Approve each action: every command, file change, web page, place outside this folder and tool from your own MCP servers, before it runs.',
    without: 'Web searches, in every mode. In Ask and Plan, no file changes, no commands and none of your own MCP tools; in Edit, commands, the web and your own MCP tools, and nothing outside this folder. In Auto, everything.',
    always: KEPT_BY_LOCUST
  },
  {
    runtime: 'copilot',
    reach: 'each-action',
    asks: 'In Approve each action: every command and file change, and each fetch or place outside this folder, before it runs.',
    without: 'Reading files, in every mode. In Ask and Plan, no file changes and no commands; in Edit, commands and changes in this folder. In Auto, everything.',
    always: KEPT_BY_LOCUST
  },
  {
    runtime: 'cursor',
    reach: 'nothing',
    asks: 'Nothing: Cursor’s command line has no way to stop and ask Locust.',
    without: 'In Ask and Plan, Cursor’s own read-only mode. In Edit, changes in this folder, and commands as Cursor’s own settings allow. In Auto, everything.'
  },
  {
    runtime: 'antigravity',
    reach: 'questions-only',
    asks: 'Only its questions, which you answer on a card. It never asks before an action.',
    without: 'In Ask and Plan, no file changes and no commands; in Edit, file changes and no commands; in Auto, everything. What it refused is listed after the run.'
  },
  {
    runtime: 'muse',
    reach: 'nothing',
    asks: 'Nothing: Muse runs without asking Locust.',
    without: 'In Ask and Plan, no file changes and no commands. In Edit, changes in this folder, commands inside Muse’s own sandbox, and the web. In Auto, everything, with commands outside that sandbox.'
  },
  {
    // 0.777: any agent that speaks the Agent Client Protocol, added by its command line. It runs in Approve each
    // action only, and every request it sends is a card -- but which actions it sends one for is its own setting
    // (OpenCode over ACP edits without asking unless its opencode.json says "ask"), so not "each action".
    runtime: 'acp',
    reach: 'some-actions',
    asks: 'In Approve each action, its only mode: every action the agent asks about, before it runs. Which actions it asks about is the agent’s own setting.',
    without: 'Whatever its own settings let it do without asking, in the folder it was given.',
    always: KEPT_BY_LOCUST,
    asksBrief: 'what the agent asks about'
  }
]

/** The lede, said the same way in Settings and in the document. */
export const WHAT_LOCUST_CAN_STOP_LEDE =
  'Locust can stop a run only where its AI agent asks first. Where it asks, a card waits for you, your saved rules answer before it reaches you, and the record says who decided. Where it does not, the mode decides what the agent is given.'

/** docs/WHAT-LOCUST-CAN-STOP.md, written from the rows above. */
export function whatLocustCanStopDocument(): string {
  const lines = [
    '# What Locust can stop',
    '',
    '<!-- Written from apps/desktop/src/shared/what-locust-can-stop.ts. Edit that file, then run',
    '     `npx vitest run src/main/what-locust-can-stop-is-what-it-asks.test.ts -u` in apps/desktop to rewrite this one. -->',
    '',
    WHAT_LOCUST_CAN_STOP_LEDE,
    '',
    '| AI agent | Can stop | Asks first | Always kept by |',
    '| --- | --- | --- | --- |',
    ...WHAT_LOCUST_CAN_STOP.map((row) => `| ${runtimeDisplayName(row.runtime)} | ${STOP_REACH_LABEL[row.reach]} | ${row.asksBrief ?? (row.reach === 'nothing' ? 'never' : row.reach === 'questions-only' ? 'its questions' : row.reach === 'each-action' ? 'in Approve each action' : 'connectors, and commands in Edit')} | ${row.always === undefined ? '—' : row.always === KEPT_BY_LOCUST ? 'Locust' : runtimeDisplayName(row.runtime)} |`),
    ''
  ]
  for (const row of WHAT_LOCUST_CAN_STOP) {
    lines.push(`## ${runtimeDisplayName(row.runtime)}`, '', `**Asks first.** ${row.asks}`, '', `**Without asking.** ${row.without}`, '')
    if (row.always !== undefined) lines.push(`**Always.** ${row.always}`, '')
  }
  return lines.join('\n')
}
