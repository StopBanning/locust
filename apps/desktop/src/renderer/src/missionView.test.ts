import type { ActivityDetail } from './missionView.js'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import type { PublicPeerMessage, PublicRecoveredMission } from '../../shared/ipc.js'
import { describe, expect, it } from 'vitest'
import { withAttachments } from '../../shared/attachments.js'

import { netFileEntries,
  activityCounts,
  readPlan,
  activityEntries,
  activitySummary,
  assistantMessages,
  turnText,
  buildSignalRail,
  buildThread,
  editToolName,
  cancellationSummary,
  conversationTurns,
  decisionStanding,
  errorAlreadyShown,
  boundedShellOutput,
  MAX_SHELL_OUTPUT_LINES,
  SHELL_OUTPUT_HEAD_LINES,
  SHELL_OUTPUT_TAIL_LINES,
  turnAttachments,
  turnPromptLine,
  peerRunFor,
  defaultOpenEntry,
  failureMessage,
  peerExchangeStartsOpen,
  peerGroups,
  peerSnippet,
  railLabel,
  railToolName,
  recentlyUsedRoutes,
  relativePath,
  usageWindowLabel,
  usageWindowSentence,
  usagePercent,
  usageReadOf,
  usageReadLine,
  activityTrace,
  durationText,
  traceOutcome,
  relayedTitle,
  resolvedModelNames,
  resumableSessionOf,
  rootMission,
  shellCommandText,
  startedLabel,
  stitchedHandoff,
  threadMarkers,
  threadPeerCards,
  typedPrompt
, commandsRun, commandsRunText, HANDOFF_INSTRUCTION_MARKER, isEditCommand, switchOf,
  stepsLine, displayPath, liveActionLine
} from './missionView.js'

const NOW = '2026-08-31T16:00:00.000Z'
let sequence = 0

function event(type: string, payload: Record<string, unknown>): NormalizedRuntimeEvent {
  sequence += 1
  return {
    id: `evt_${sequence}`,
    runId: 'run_1',
    missionId: 'mission_1',
    sequence,
    occurredAt: NOW,
    sourceAdapter: 'codex',
    type,
    payload: { evidence: { redacted: false }, ...payload }
  } as unknown as NormalizedRuntimeEvent
}

function startedEvent(runtimeThreadId?: string): NormalizedRuntimeEvent {
  const started = event('run.started', { runtimeThreadId: runtimeThreadId ?? '' })
  return runtimeThreadId === undefined
    ? started
    : ({ ...started, runtimeThreadId } as unknown as NormalizedRuntimeEvent)
}

function toolStart(itemId: string, name: string, command?: string): NormalizedRuntimeEvent {
  return event('tool.started', {
    itemId,
    toolKind: 'command_execution',
    name,
    phase: 'started',
    ...(command === undefined ? {} : { command })
  })
}

function toolDone(itemId: string): NormalizedRuntimeEvent {
  return event('tool.completed', {
    itemId,
    toolKind: 'command_execution',
    name: 'shell',
    phase: 'completed'
  })
}

function delta(itemId: string, text: string, operation: 'append' | 'replace', final = false): NormalizedRuntimeEvent {
  return event('message.delta', { itemId, operation, text, final })
}

describe('assistant text', () => {
  it('rebuilds an appended stream instead of keeping the last fragment', () => {
    const messages = assistantMessages([
      delta('a', 'Adapter parity ', 'append'),
      delta('a', 'holds for ', 'append'),
      delta('a', 'invoice.paid.', 'append', true)
    ])
    expect(messages).toEqual([{ itemId: 'a', text: 'Adapter parity holds for invoice.paid.', final: true }])
  })

  it('honors a replace as a replace', () => {
    const messages = assistantMessages([
      delta('a', 'partial', 'append'),
      delta('a', 'the whole answer', 'replace', true)
    ])
    expect(messages[0]?.text).toBe('the whole answer')
  })

  it('keeps separate messages separate and in order', () => {
    const messages = assistantMessages([
      delta('a', 'first', 'append', true),
      delta('b', 'second', 'append', true)
    ])
    expect(messages.map((m) => m.text)).toEqual(['first', 'second'])
  })
})

describe('how a path is written in a row', () => {
  const WS = String.raw`C:\Users\x\projects\streaks`

  it('drops the workspace, because the row is one line and the filename is the point', () => {
    expect(relativePath(String.raw`C:\Users\x\projects\streaks\src\streak.js`, WS)).toBe('src/streak.js')
  })

  it('matches case-insensitively, the way Windows does', () => {
    expect(relativePath(String.raw`c:\users\x\projects\streaks\src\cli.js`, WS)).toBe('src/cli.js')
  })

  it('draws the workspace root as the folder name, not its whole path', () => {
    // A directory-listing tool reports the folder it acted on, which is the
    // workspace root. That fell through every branch and kept its absolute
    // path, so a stopped run's FINISHED list opened with a long absolute path
    // above three bare filenames (measured 2026-09-07).
    expect(relativePath(WS, WS)).toBe('streaks')
    // A trailing separator is the same folder.
    expect(relativePath(WS + String.fromCharCode(92), WS)).toBe('streaks')
  })

  it('keeps a path outside the workspace whole, because there the location is the information', () => {
    const outside = String.raw`C:\Users\x\other\thing.js`
    expect(relativePath(outside, WS)).toBe(outside)
  })

  it('changes nothing when the workspace is unknown', () => {
    const path = String.raw`C:\Users\x\projects\streaks\src\streak.js`
    expect(relativePath(path, undefined)).toBe(path)
    expect(relativePath(path, '')).toBe(path)
  })

  it('leaves a path that is already relative alone', () => {
    expect(relativePath('src/streak.js', WS)).toBe('src/streak.js')
  })

  it("drops a teammate's own-branch tree too, because the tree is the same project", () => {
    expect(relativePath(String.raw`C:\Users\x\projects\streaks\.locust\worktrees\tm_abc123\src\streak.js`, WS)).toBe('src/streak.js')
    expect(relativePath('.locust/worktrees/tm_abc123/README.md', WS)).toBe('README.md')
  })

  it("drops a comparison column's copy the same way (0.445)", () => {
    // Seen in drive-compare-changes' frames: OpenCode named the file it edited
    // as `.locust/compare/cmp_…-a/cart.py` beside the observed `cart.py`, so
    // one changed line read as "2 files, +2 -2" under an internal path.
    expect(relativePath('.locust/compare/cmp_f7da0ffad7e54dd8-a/cart.py', WS)).toBe('cart.py')
    expect(relativePath(String.raw`C:\Users\x\projects\streaks\.locust\compare\cmp_1-b\src\streak.js`, WS)).toBe('src/streak.js')
  })

  it("drops a routine's copy the same way (0.534)", () => {
    // Seen in drive-a-routine-until-its-check-passes: turn 1 listed answer.txt
    // twice, once under the copy's home path.
    expect(relativePath(String.raw`C:\Users\x\.locust\routines\rt_goal0000\answer.txt`, WS)).toBe('answer.txt')
    expect(relativePath('/home/x/.locust/routines/rt_abc/docs/plan.md', WS)).toBe('docs/plan.md')
  })

  it("drops a comparison column's plain copy outside the folder too (0.448)", () => {
    expect(relativePath(String.raw`C:\Users\x\.locust\compare\cmp_ab12-b\site\index.html`, WS)).toBe('site/index.html')
    // Anything else outside the folder keeps its whole path: there the place IS the information.
    expect(relativePath(String.raw`C:\Users\x\.locust\other\index.html`, WS)).toBe(String.raw`C:\Users\x\.locust\other\index.html`)
  })

  it('keeps the tree root itself, since there is nothing shorter that is true', () => {
    expect(relativePath(String.raw`C:\Users\x\projects\streaks\.locust\worktrees\tm_abc123`, WS)).toBe('.locust/worktrees/tm_abc123')
  })
})

describe('when a mission says it began', () => {
  const now = new Date('2026-09-03T14:00:00')

  it('shows the bare time for a mission started today', () => {
    // The locale decides 24-hour or AM/PM; what matters is that no date rides
    // along on a mission from today.
    const label = startedLabel('2026-09-03T09:15:00', now)
    expect(label).toContain('09:15')
    expect(label).not.toMatch(/Sep|\d{4}/)
  })

  it('carries the date once the mission is not from today', () => {
    // `started 12:25 AM` with no date is unambiguous only while the app stays
    // open; the next morning a mission from last night reads as recent.
    const label = startedLabel('2026-09-02T23:25:00', now)
    expect(label).toContain('Sep')
    expect(label).toContain('2')
  })

  it('adds the year only when the mission is from another one', () => {
    expect(startedLabel('2025-12-31T23:59:00', now)).toContain('2025')
    expect(startedLabel('2026-09-01T10:00:00', now)).not.toContain('2026')
  })

  it('says nothing for a timestamp it cannot read', () => {
    expect(startedLabel('not a date', now)).toBeUndefined()
  })
})

describe('which tool names mean a file was touched (0.35.2 QA)', () => {
  it('does not count a to-do list or a sub-agent writer as a file', () => {
    // OpenCode's `todowrite` is the model's own to-do list and is offered
    // even to read-only runs; Copilot's `write_agent` starts a helper. Both
    // matched a bare `write` and the fold reported a changed file on a run
    // that changed nothing.
    expect(editToolName('todowrite')).toBe(false)
    expect(editToolName('write_agent')).toBe(false)
    expect(editToolName('todoread')).toBe(false)
  })

  it('counts a removal, which Cursor calls delete', () => {
    // The same mistake from the other side: a run that deleted a file
    // reported a tool call and no file.
    expect(editToolName('delete')).toBe(true)
    expect(editToolName('deleteFile')).toBe(true)
  })

  it('still counts the ordinary ones', () => {
    for (const name of ['write', 'Write', 'edit', 'apply_patch', 'file_change', 'create_file', 'rename']) {
      expect(editToolName(name)).toBe(true)
    }
    for (const name of ['read', 'grep', 'glob', 'shell', 'web_search']) {
      expect(editToolName(name)).toBe(false)
    }
  })
})

describe('what the fold counts as a changed file (0.35.0 QA)', () => {
  // Ported from the 0.35.0 targeted QA's own regression file. Three of its
  // four cases failed on 0.35.1; each is paired here with the control that
  // stops the fix from being "always answer one".
  const patchFor = (path: string) => ({
    text: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1 +1 @@\n-draft\n+final\n`,
    added: 1,
    removed: 1,
    truncated: false
  })
  const filesText = (details: readonly ActivityDetail[]): string | undefined =>
    activityTrace(details, [], 'completed').find((segment) => segment.key === 'files')?.text

  it('counts one Copilot multiline patch as one changed file', () => {
    expect(
      filesText([
        {
          kind: 'edit',
          name: '*** Begin Patch\n*** Update File: notes.ts\n@@\n-draft\n+final\n*** End Patch',
          settled: true,
          patch: patchFor('notes.ts')
        }
      ])
    ).toBe('1 file')
  })

  it('counts two edits to the same path once', () => {
    const detail: ActivityDetail = { kind: 'edit', name: 'notes.ts', settled: true, patch: patchFor('notes.ts') }
    expect(filesText([detail, detail])).toBe('1 file')
  })

  it('still counts two edits to two paths as two', () => {
    expect(
      filesText([
        { kind: 'edit', name: 'notes.ts', settled: true, patch: patchFor('notes.ts') },
        { kind: 'edit', name: 'other.ts', settled: true, patch: patchFor('other.ts') }
      ])
    ).toBe('2 files')
  })

  it('does not count a refused write as a changed file', () => {
    expect(filesText([{ kind: 'edit', name: 'blocked.txt', settled: true, failed: true }])).toBeUndefined()
  })

  it('still counts the writes that landed beside a refused one', () => {
    expect(
      filesText([
        { kind: 'edit', name: 'blocked.txt', settled: true, failed: true },
        { kind: 'edit', name: 'notes.ts', settled: true, patch: patchFor('notes.ts') }
      ])
    ).toBe('1 file')
  })
})

describe('the words a person typed, across a route switch (0.35.0 QA)', () => {
  const mission = (fields: Record<string, unknown>): PublicRecoveredMission =>
    ({ peerMessages: [], ...fields }) as unknown as PublicRecoveredMission

  it('keeps the new instruction a handoff was started for', () => {
    // `main/handoff.ts` appends the person's next words LAST, as its own
    // section; walking back past it showed the original task instead.
    const first = mission({ missionId: 'first', prompt: 'Create a module and test it' })
    const next = mission({
      missionId: 'next',
      prompt: 'Another agent started this task.\n\nThe person now asks:\n\nRun its tests without editing.',
      continuesFrom: { missionId: 'first', checkpointEpoch: 1, reason: 'route-switch' }
    })
    expect(typedPrompt(next, new Map([['first', first], ['next', next]]))).toBe('Run its tests without editing.')
  })

  it("leaves a person's own words alone, even when they contain the marker", () => {
    // Someone working on this codebase types the sentence, and the mission
    // was titled with whatever followed it (QA, 2026-09-06). A mission a
    // person typed is their words already, whatever it happens to contain.
    const typed = mission({
      missionId: 'typed',
      prompt: 'Rename the string "The person now asks:" in handoff.ts and update both readers.'
    })
    expect(typedPrompt(typed, new Map([['typed', typed]]))).toBe(
      'Rename the string "The person now asks:" in handoff.ts and update both readers.'
    )
  })

  it('does not read the marker out of a briefing that quotes the original task', () => {
    // A rescue quotes the task it is continuing. When the task itself held
    // the sentence, the match landed inside the quote and the briefing's own
    // sections were shown as the person's words.
    const first = mission({
      missionId: 'first',
      prompt: 'Rename the string "The person now asks:" in handoff.ts and update both readers.'
    })
    const rescue = mission({
      missionId: 'rescue',
      prompt:
        'Another agent started this task:\n\nRename the string "The person now asks:" in handoff.ts and update both readers.\n\nThese actions reported finishing before the stop: none.',
      continuesFrom: { missionId: 'first', checkpointEpoch: 1, reason: 'route-switch' }
    })
    expect(typedPrompt(rescue, new Map([['first', first], ['rescue', rescue]]))).toBe(
      'Rename the string "The person now asks:" in handoff.ts and update both readers.'
    )
  })

  it('still shows the original words when the switch carried no new instruction', () => {
    // The rescue case: a route switch with nothing new to say must not start
    // showing the host's briefing, which is what this walk-back exists for.
    const first = mission({ missionId: 'first', prompt: 'Create a module and test it' })
    const next = mission({
      missionId: 'next',
      prompt: 'Another agent started this task. Continue from its checkpoint.',
      continuesFrom: { missionId: 'first', checkpointEpoch: 1, reason: 'route-switch' }
    })
    expect(typedPrompt(next, new Map([['first', first], ['next', next]]))).toBe('Create a module and test it')
  })
})

describe("OpenCode's plan, in the shape it really arrives in", () => {
  /*
   * The exact payload a live opencode run sent on 2026-09-13 -- three steps,
   * spelled `content`, with `in_progress` between pending and completed. The
   * adapter test proves the events; this proves the THREAD reads them.
   *
   * Until this build `readPlan` knew `step`/`text`/`title`/`name` and not
   * `content`, so these steps would have arrived and drawn nothing: a plan
   * with three rows of blank.
   */
  const OPENCODE_PLAN = [
    { id: '1', content: 'create a.txt containing alpha', status: 'completed' },
    { id: '2', content: 'create b.txt containing beta', status: 'in_progress' },
    { id: '3', content: 'create c.txt listing the two file names', status: 'pending' }
  ]

  it('reads the steps, their words and their states', () => {
    const steps = readPlan(OPENCODE_PLAN)
    expect(steps.map((step) => step.text)).toEqual([
      'create a.txt containing alpha',
      'create b.txt containing beta',
      'create c.txt listing the two file names'
    ])
    expect(steps.map((step) => step.state)).toEqual(['done', 'running', 'pending'])
  })

  it('and the thread counts how far through it is, in the open', () => {
    const thread = buildThread(
      [event('tool.started', { itemId: 't1', name: 'write' }), event('plan.updated', { plan: OPENCODE_PLAN })],
      { running: true }
    )
    const shown = thread.find((item) => item.type === 'plan')
    expect(shown?.type === 'plan' ? shown.steps.length : undefined).toBe(3)
    expect(shown?.type === 'plan' ? shown.doneCount : undefined).toBe(1)
  })
})

describe('a plan is WHAT, and the fold is HOW', () => {
  /*
   * This used to read "a plan is what the run did, not a thing above what
   * it did", and the plan rode inside the activity fold whenever there was
   * one. The reasoning then was about card proliferation: nineteen visual
   * species against Claude Code's three, and no rule for what earns a card
   * (design review, 2026-09-06). Fair, and it put the plan in the wrong
   * place.
   *
   * Colin, 2026-09-14, watching a five-step run: "the plan should be
   * visible to the user probably". He is right, and the line is the one the
   * trace comment already draws -- the fold hides HOW a turn was carried
   * out, and a plan is WHAT it is doing and how far along. That is the only
   * question a person waiting actually has, and it was the one thing they
   * had to go looking for.
   *
   * Not a new species either: the `plan` item already existed for turns
   * that touched nothing. It simply stopped being conditional.
   */
  const steps = { plan: [{ step: 'Read the file', status: 'completed' }, { step: 'Edit it', status: 'in_progress' }] }

  it('stands on its own even when the run also did something', () => {
    const thread = buildThread(
      [
        event('plan.updated', steps),
        event('tool.started', { itemId: 'i1', name: 'edit', command: 'src/notes.ts' }),
        event('tool.completed', { itemId: 'i1', name: 'edit', command: 'src/notes.ts', status: 'ok' }),
        event('run.completed', {})
      ],
      { running: false, latestTurn: true, spokeToPeers: false }
    )
    const shown = thread.find((item) => item.type === 'plan')
    expect(shown, 'the plan is its own item, not a passenger on the fold').toBeDefined()
    expect(shown?.type === 'plan' ? shown.steps.length : undefined).toBe(2)
    // The fold is still there, and still carries the work.
    const fold = thread.find((item) => item.type === 'activity')
    expect(fold).toBeDefined()
    // Read first: the shape of the work, then how it was carried out.
    expect(thread.indexOf(shown!)).toBeLessThan(thread.indexOf(fold!))
    // The count travels with the plan, which is now the only place it is.
    expect(shown?.type === 'plan' ? shown.doneCount : undefined).toBe(1)
  })

  it('keeps its own item when the run did nothing else', () => {
    // The control, and it is a real state: a read-only run that answers with
    // steps and touches nothing has no fold to ride on, and a plan that
    // vanished would leave the turn with no account at all.
    const thread = buildThread(
      [event('plan.updated', steps), event('run.completed', {})],
      { running: false, latestTurn: true, spokeToPeers: false }
    )
    expect(thread.some((item) => item.type === 'plan')).toBe(true)
    expect(thread.some((item) => item.type === 'activity')).toBe(false)
  })
})

describe('a turn whose whole answer was a message to a teammate', () => {
  // Those messages are drawn beside the thread, not inside it, so from in
  // here the turn looked like one that said nothing -- and the warning landed
  // directly above the message it had just sent (Colin, 2026-09-06: "the this
  // turn ended with a reply intended?").
  const finished = [event('run.completed', {})]

  it('is not reported as a turn that said nothing', () => {
    const thread = buildThread(finished, { running: false, latestTurn: true, spokeToPeers: true })
    expect(thread.some((item) => item.type === 'diagnostic')).toBe(false)
  })

  it('still says so when the runtime really wrote nothing at all', () => {
    const thread = buildThread(finished, { running: false, latestTurn: true, spokeToPeers: false })
    const said = thread.find((item) => item.type === 'diagnostic')
    expect(said?.type === 'diagnostic' && said.message).toMatch(/ended without a reply/)
  })
})

describe('a row a runtime names only when the tool finishes', () => {
  it('takes the target from the completion when the start had none', () => {
    // Claude streams a tool's input after the call opens, so the row read
    // `Read` with no file until this arrived.
    const thread = buildThread(
      [
        event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'Read', phase: 'started' }),
        event('tool.completed', { itemId: 't1', toolKind: 'tool_use', name: 'Read', command: 'src/cli.js', phase: 'completed' })
      ],
      { running: false }
    )
    const activity = thread.find((item) => item.type === 'activity')
    expect(activity?.type === 'activity' && activity.details[0]?.name).toBe('src/cli.js')
    expect(activity?.type === 'activity' && activity.details[0]?.tool).toBe('Read')
  })

  it('leaves a row the start already named alone', () => {
    const thread = buildThread(
      [
        event('tool.started', { itemId: 't1', toolKind: 'command_execution', name: 'shell', command: 'npm test', phase: 'started' }),
        event('tool.completed', { itemId: 't1', toolKind: 'command_execution', name: 'shell', command: 'something else', phase: 'completed' })
      ],
      { running: false }
    )
    const activity = thread.find((item) => item.type === 'activity')
    expect(activity?.type === 'activity' && activity.details[0]?.name).toBe('npm test')
  })
})

describe('the command a shell row shows', () => {
  const PS = String.raw`"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"`

  it('shows what ran, not the host that ran it', () => {
    // The row is one line wide. With the host in front, every command read as
    // the same truncated path and the actual work was cut off.
    expect(shellCommandText(`${PS} -Command "npm test"`)).toBe('npm test')
    expect(shellCommandText(`${PS} -NoProfile -NonInteractive -Command 'node --test'`)).toBe('node --test')
  })

  it('keeps a command that was never wrapped', () => {
    expect(shellCommandText('pnpm build')).toBe('pnpm build')
    expect(shellCommandText('git diff -- src/streak.js')).toBe('git diff -- src/streak.js')
  })

  it('unwraps a cmd.exe host too', () => {
    expect(shellCommandText(String.raw`C:\Windows\System32\cmd.exe /d /s /c "npm run build"`)).toBe('npm run build')
  })

  it('undoes the quote doubling the host introduced', () => {
    expect(shellCommandText(`${PS} -Command "rg -n ""streak"" src"`)).toBe('rg -n "streak" src')
  })
})

describe('what a failure card says', () => {
  const nl = String.fromCharCode(10)

  it("shows the runtime's own last word, because the host's sentence names only the shape", () => {
    // Measured 2026-09-03: this exact pair cost two runs that read on screen
    // as the same shrug.
    expect(
      failureMessage({
        message: 'Codex invocation did not complete successfully',
        process: { stderr: 'Not inside a trusted directory and --skip-git-repo-check was not specified.' + nl }
      })
    ).toBe(
      "Codex invocation did not complete successfully The runtime's own last word was: Not inside a trusted directory and --skip-git-repo-check was not specified."
    )
  })

  it('says capacity exhaustion in English rather than passing the jargon through alone', () => {
    const said = failureMessage({
      message: 'Cursor Agent ended without a terminal result record.',
      process: { stderr: 'RetriableError: [resource_exhausted] Error' + nl }
    })
    expect(said).toContain('out of capacity right now')
    // The runtime's own text still rides along: a person reporting this
    // upstream needs the words upstream uses.
    expect(said).toContain('resource_exhausted')
  })

  // 0.823: the 2026-10-22 handoff drives' cards said Muse's quota three times and pasted Antigravity's raw AGY_ERROR JSON.
  it('says a limit once when the message already names it in plain words', () => {
    const quota = 'API error 429 [request_id=2370cf39]: Subscription quota exhausted. Your usage window resets at 2026-10-26T00:00:00Z. (rate_limit_error)'
    expect(failureMessage({
      message: "Muse Code's plan is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.",
      process: { stderr: `muse: Agent delegation: auto unavailable: workspace is untrusted.${nl}run ended with Failed: ${quota}${nl}` }
    }, 'muse')).toBe("Muse Code's plan is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.")
    const agy = 'Antigravity could not run it: Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s.'
    expect(failureMessage({
      message: agy,
      process: { stderr: `AGY_ERROR: {"short_error":"RESOURCE_EXHAUSTED (code 429): Individual quota reached.","status":"RESOURCE_EXHAUSTED","error_code":429}${nl}` }
    }, 'antigravity')).toBe(agy)
    // And the sentence Antigravity's quota now ends with (the packaged 0.823 drive), with the same runtime line.
    const sentence = "Antigravity's quota is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets."
    expect(failureMessage({
      message: sentence,
      process: { stderr: `AGY_ERROR: {"short_error":"RESOURCE_EXHAUSTED (code 429): Individual quota reached.","status":"RESOURCE_EXHAUSTED","error_code":429}${nl}` }
    }, 'antigravity')).toBe(sentence)
    // A message that does not name its limit still gets the runtime's own words, as before.
    expect(failureMessage({ message: 'Cursor Agent ended without a terminal result record.', process: { stderr: `RetriableError: [resource_exhausted] Error${nl}` } }))
      .toContain('out of capacity right now')
  })

  it('adds nothing when the runtime said nothing', () => {
    expect(failureMessage({ message: 'Codex CLI is not ready.' })).toBe('Codex CLI is not ready.')
    expect(failureMessage({ message: 'Codex CLI is not ready.', process: { stderr: '   ' + nl } })).toBe(
      'Codex CLI is not ready.'
    )
  })

  it('strips the terminal colour codes a runtime writes around its own words', () => {
    // Colin, 2026-09-05: OpenCode's refusal reached the card as three boxes
    // before the sentence a person needs to read. ESC is built from its code
    // so an editor eating an invisible character cannot break this quietly.
    const esc = String.fromCharCode(27)
    const message = failureMessage({
      message: 'OpenCode ended without a step that reported it had stopped.',
      process: {
        stderr: esc + '[93m' + esc + '[1m! ' + esc + '[0mpermission requested: external_directory; auto-rejecting'
      }
    })
    expect(message).toBe(
      "OpenCode ended without a step that reported it had stopped. The runtime's own last word was: ! permission requested: external_directory; auto-rejecting"
    )
    expect(message).not.toContain(esc)
    expect(message).not.toContain('[93m')
  })

  it('still finds the last SPEAKING line when the final one is only colour codes', () => {
    const esc = String.fromCharCode(27)
    expect(
      failureMessage({
        message: 'Codex stopped.',
        process: { stderr: 'the disk is full' + String.fromCharCode(10) + esc + '[0m' + esc + '[2K' }
      })
    ).toBe("Codex stopped. The runtime's own last word was: the disk is full")
  })

})

describe("a runtime's own helper", () => {
  it("is its own row: what it was asked, and whether it reported back", () => {
    const thread = buildThread(
      [
        event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'Task', command: 'Search the tests for flaky cases', phase: 'started' }),
        event('tool.completed', { itemId: 't1', toolKind: 'tool_use', name: 'Task', command: 'Search the tests for flaky cases', phase: 'completed' }),
        event('tool.started', { itemId: 't2', toolKind: 'task', name: 'task', command: 'Summarise README', phase: 'started' })
      ],
      { running: true }
    )
    const activity = thread.find((item) => item.type === 'activity')
    expect(activity?.type === 'activity' && activity.details.map((d) => d.kind)).toEqual(['helper', 'helper'])
    const entries = activity?.type === 'activity' ? activityEntries(activity.details) : []
    expect(entries.map((e) => (e.kind === 'helper' ? [e.description, e.settled] : e.kind))).toEqual([
      ['Search the tests for flaky cases', true],
      ['Summarise README', false]
    ])
    expect(activity?.type === 'activity' && activity.summary).toBe('asked 2 subagents')
  })

  it('a helper the runtime never named is said to be unnamed, not drawn as a path', () => {
    const entries = activityEntries([{ kind: 'helper', name: 'Task', tool: 'Task', settled: true }])
    expect(entries).toEqual([{ kind: 'helper', key: 'helper_0', description: 'a subagent, unnamed', settled: true, failed: false }])
  })
})

describe('collapsed activity', () => {
  it('counts edits and commands separately from their tool events', () => {
    expect(
      activitySummary([
        { kind: 'edit', name: 'apply_patch', settled: true },
        { kind: 'edit', name: 'apply_patch', settled: true },
        { kind: 'shell', name: 'pnpm test', settled: true }
      ])
    ).toBe('Edited 2 files · ran 1 command')
  })

  it('counts files, not edit calls: one Codex file_change can name several', () => {
    expect(activitySummary([{ kind: 'edit', name: 'C:/w/README.md' + String.fromCharCode(10) + 'C:/w/src/prices.ts', settled: true }])).toBe('Edited 2 files')
  })

  it('draws one row per file a runtime named without a diff, never one row named after the tool', () => {
    const rows = activityEntries([
      // The real shape: the paths arrive as the tool's command and become the
      // detail's name, while the tool stays the literal 'file_change'.
      { kind: 'edit', name: 'C:/w/README.md' + String.fromCharCode(10) + 'C:/w/src/prices.ts', tool: 'file_change', settled: true }
    ])
    expect(rows.map((row) => [row.kind, 'name' in row ? row.name : ''])).toEqual([
      ['unreported', 'C:/w/README.md'],
      ['unreported', 'C:/w/src/prices.ts']
    ])
  })

  it('singularizes honestly', () => {
    expect(activitySummary([{ kind: 'edit', name: 'x', settled: true }])).toBe('Edited 1 file')
    // A refused write edited nothing; it is a call, and its row says failed.
    expect(activitySummary([{ kind: 'edit', name: 'C:/Users/x/.claude/plans/p.md', settled: true, failed: true }])).toBe('1 tool call')
  })

  // M24 (the code review): every sed counted as an edit, so a read-only
  // `sed -n` was "Edited 1 file", named after the command.
  it('counts sed as an edit only in place, and tee only with a file', () => {
    const read = buildThread([toolStart('t1', 'shell', "sed -n '1,40p' src/app.ts")], { running: true })
    expect(read.find((item) => item.type === 'activity')).toMatchObject({ summary: 'ran 1 command' })
    const inPlace = buildThread([toolStart('t1', 'shell', "sed -i 's/a/b/' src/app.ts")], { running: true })
    expect(inPlace.find((item) => item.type === 'activity')).toMatchObject({ summary: 'Edited 1 file' })
    expect(isEditCommand("sed -i.bak 's/a/b/' x")).toBe(true)
    expect(isEditCommand("sed --in-place 's/a/b/' x")).toBe(true)
    expect(isEditCommand("sed -E 's/a/b/g' x")).toBe(false)
    expect(isEditCommand('tee out.log')).toBe(true)
    expect(isEditCommand('tee -a out.log')).toBe(true)
    expect(isEditCommand('tee')).toBe(false)
  })

  it('says so when there was no tool activity', () => {
    expect(activitySummary([])).toBe('No tool activity')
  })

  it('classifies a patch command as an edit and a test run as a command', () => {
    const thread = buildThread(
      [toolStart('t1', 'shell', 'apply_patch <<EOF'), toolStart('t2', 'shell', 'pnpm test')],
      { running: true }
    )
    const activity = thread.find((item) => item.type === 'activity')
    expect(activity).toMatchObject({ summary: 'Edited 1 file · ran 1 command' })
  })
})

describe('thread composition', () => {
  it('shows a caret only while text is genuinely still arriving', () => {
    const streaming = buildThread([delta('a', 'partial', 'append')], { running: true })
    const finished = buildThread([delta('a', 'done', 'append', true)], { running: true })
    const stopped = buildThread([delta('a', 'partial', 'append')], { running: false })
    expect(streaming.find((i) => i.type === 'agent-message')).toMatchObject({ streaming: true })
    // Final means the provider is done with this message even if the run is not.
    expect(finished.find((i) => i.type === 'agent-message')).toMatchObject({ streaming: false })
    expect(stopped.find((i) => i.type === 'agent-message')).toMatchObject({ streaming: false })
  })

  it('drops the live step once it completes', () => {
    const running = buildThread([event('step.started', { stepKind: 'turn', message: 'Running the billing suite' })], {
      running: true
    })
    const done = buildThread(
      [
        event('step.started', { stepKind: 'turn', message: 'Running the billing suite' }),
        event('step.completed', { stepKind: 'turn' })
      ],
      { running: true }
    )
    expect(running.find((i) => i.type === 'live-step')).toMatchObject({ label: 'Running the billing suite' })
    // The NAMED step goes the moment it completes -- a finished step must
    // never sit there looking live. What replaces it is a generic waiting
    // line, because the run is still up: `liveActivityOf` calls that state
    // "working", and the thread has to say the same thing the face does.
    expect(done.find((i) => i.type === 'live-step')).toMatchObject({ label: 'Working', waiting: true })
  })

  it('shows a line the moment a run starts, before any event arrives', () => {
    // The gap this closes: pressing Enter drew nothing at all until the
    // runtime's first event, which for a CLI that has to launch a process is
    // seconds of blank page (Colin, twice: "the input to working/thinking lag
    // still feels clunky").
    const justSent = buildThread([], { running: true, startedAt: '2026-09-04T21:47:00.000Z' })
    expect(justSent.find((i) => i.type === 'live-step')).toMatchObject({
      label: 'Starting',
      waiting: true,
      startedAt: '2026-09-04T21:47:00.000Z'
    })
  })

  it('says nothing above an approval the run is stopped on', () => {
    // A run awaiting a decision is still `running`. A "Working" line directly
    // above the card asking the question would contradict the header, which
    // resolves the same teammate to "waiting on you".
    const items = buildThread([], {
      running: true,
      startedAt: '2026-09-04T21:47:00.000Z',
      awaitingDecision: true
    })
    expect(items.some((i) => i.type === 'live-step')).toBe(false)
  })

  it('says nothing above an approval even with a tool open -- the tool IS what is waiting', () => {
    // Antigravity's ask_question stays open until the person answers, and a
    // Codex command stays open while its approval card is up. The line above
    // the card read "Using a tool..." with a working face while the header
    // said "waiting on you" (drive-antigravity-answer, 2026-09-23).
    const opened = event('tool.started', { itemId: 'tool_7_0', toolKind: 'ask_question', name: 'ask_question', command: 'How would you like to organize this folder?', phase: 'started' })
    const items = buildThread([opened], { running: true, startedAt: '2026-09-23T04:43:02.000Z', awaitingDecision: true })
    expect(items.some((i) => i.type === 'live-step')).toBe(false)
    // The control: the same open tool with nobody asked draws its line.
    expect(buildThread([opened], { running: true, startedAt: '2026-09-23T04:43:02.000Z' }).some((i) => i.type === 'live-step')).toBe(true)
  })

  it('draws no live line once the run is over', () => {
    expect(buildThread([], { running: false, startedAt: '2026-09-04T21:47:00.000Z' })).toEqual([])
  })

  it('does not put a waiting line under a message that is still streaming', () => {
    // Text arriving IS the teammate doing something visible; a "Working" line
    // under it would say the opposite of what the reader can see.
    const items = buildThread([delta('a', 'half a sen', 'append')], { running: true, startedAt: '2026-09-04T21:47:00.000Z' })
    const live = items.find((i) => i.type === 'live-step')
    expect(live).not.toMatchObject({ waiting: true })
    expect(live).not.toMatchObject({ register: 'working' })
  })

  it('says it is writing while the reply arrives, on the turn clock (0.581)', () => {
    // It drew nothing at all, so the clock and the line went away for as long
    // as the reply took; Claude Code keeps its status line through a reply.
    const items = buildThread([delta('a', 'half a sen', 'append')], { running: true, startedAt: '2026-09-04T21:47:00.000Z' })
    expect(items.find((i) => i.type === 'live-step')).toMatchObject({ label: 'Writing', register: 'writing', startedAt: '2026-09-04T21:47:00.000Z' })
    // Control: once the text is final and the run is still going, it is back to working.
    const after = buildThread([delta('a', 'half a sentence.', 'append', true)], { running: true, startedAt: '2026-09-04T21:47:00.000Z' })
    expect(after.find((i) => i.type === 'live-step')).toMatchObject({ register: 'working' })
  })

  it('never shows a live step for a run that is not running', () => {
    const thread = buildThread([event('step.started', { stepKind: 'turn', message: 'Working' })], { running: false })
    expect(thread.some((i) => i.type === 'live-step')).toBe(false)
  })

  it('surfaces provider limits and diagnostics as their own items', () => {
    const thread = buildThread(
      [
        // Work has begun, so a notice here is about the mission, not the setup.
        toolStart('t1', 'shell', 'pnpm test'),
        event('route.limit_detected', { kind: 'temporary-rate-limit', message: 'Slow down' }),
        event('adapter.diagnostic', { level: 'warning', code: 'x', message: 'Heads up', terminal: false })
      ],
      { running: true }
    )
    expect(thread.find((i) => i.type === 'limit')).toMatchObject({ kind: 'temporary-rate-limit' })
    expect(thread.find((i) => i.type === 'diagnostic')).toMatchObject({ level: 'warning' })
  })

  it('finds the run a peer message was delivered into, so the exchange can be opened', () => {
    // Colin, 2026-09-04: a relayed run should be reachable "from the exchange
    // card in the thread the person is actually in". The same messageId is
    // `posted` on the run that wrote it and `received` on the run it reached.
    const link = (messageId: string, direction: 'received' | 'posted'): PublicPeerMessage => ({
      messageId,
      direction,
      from: { teammateId: 'tm_booty', name: 'Booty' },
      to: { teammateId: 'tm_wren', name: 'Wren' },
      text: 'How is your day?',
      at: NOW
    })
    const asked = { missionId: 'mission_booty', peerMessages: [link('wm_1', 'posted')] } as unknown as PublicRecoveredMission
    const answered = { missionId: 'mission_wren', peerMessages: [link('wm_1', 'received')] } as unknown as PublicRecoveredMission
    const unrelated = { missionId: 'mission_other', peerMessages: [link('wm_2', 'received')] } as unknown as PublicRecoveredMission

    expect(peerRunFor('wm_1', [asked, answered, unrelated])?.missionId).toBe('mission_wren')
    // Not the run that WROTE it -- that is the thread you are already in.
    expect(peerRunFor('wm_1', [asked])).toBeUndefined()
    // Still waiting for that teammate's next run is a real state, not an error.
    expect(peerRunFor('wm_3', [asked, answered])).toBeUndefined()
    expect(peerRunFor('', [asked, answered])).toBeUndefined()
  })

  it('never draws a turn the HOST briefed as the person\'s own words', () => {
    // MEASURED 2026-09-05, Colin's screenshot: the whole relay briefing --
    // "end with one <locust-share to=\"Wren\"> block... Do not use a
    // <locust-ask> block here" -- sat in the thread in the place a person's
    // message goes, as the most prominent text on screen.
    const briefing =
      'Wren (Code & Migrations) replied to you; it is quoted below. Do what it asks... end with one <locust-share to="Wren"> block holding your reply.'
    const received: PublicPeerMessage = {
      messageId: 'wm_1',
      direction: 'received',
      from: { teammateId: 'tm_wren', name: 'Wren' },
      to: { teammateId: 'tm_booty', name: 'Booty' },
      text: "Day's going well on my side.",
      at: '2026-09-05T00:00:00.000Z'
    }
    // The message IS drawn -- as a peer card, in time, before the work -- so
    // the bubble stands down rather than repeating it in the person's own
    // bubble style, which read as the person having said it (Colin,
    // 2026-09-06, on seeing a teammate's poem in his own bubble).
    expect(
      turnPromptLine({ prompt: briefing, startedBy: { kind: 'relay', hop: 2 }, peerMessages: [received] })
    ).toBeUndefined()

    // The record no longer holds the message: still nothing, and for the same
    // reason it has always been nothing here -- the briefing is the host's
    // words, never the person's.
    expect(turnPromptLine({ prompt: briefing, startedBy: { kind: 'relay', hop: 2 }, peerMessages: [] })).toBeUndefined()

    // The point of the whole case, and the thing that must never regress: in
    // NEITHER state does the host's briefing reach the screen.
    for (const messages of [[received], []]) {
      const line = turnPromptLine({ prompt: briefing, startedBy: { kind: 'relay', hop: 2 }, peerMessages: messages })
      expect(line ?? '').not.toContain('locust-share')
    }
    expect(turnPromptLine({ prompt: 'resumed briefing', startedBy: { kind: 'resume', epoch: 2 } })).toBeUndefined()

    // A person's words are theirs, and so are a routine's steps -- they were
    // typed by the person in the conversation the routine was saved from.
    expect(turnPromptLine({ prompt: 'read status.ts' })).toBe('read status.ts')
    expect(
      turnPromptLine({ prompt: 'read status.ts', startedBy: { kind: 'routine', routineId: 'rt_1', step: 2 } })
    ).toBe('read status.ts')
  })

  it('says so when a turn finished and wrote nothing back', () => {
    // MEASURED 2026-09-05 while Colin watched a live test: a follow-up on
    // Cursor completed cleanly, spent tokens, recorded reasoning, and emitted
    // no assistant text. The thread drew the person's message and then blank
    // space under a header saying `completed`.
    const silent = buildThread([event('run.started', {}), event('run.completed', { process: {} })], { running: false })
    expect(silent.filter((item) => item.type === 'diagnostic').map((item) => item.message)).toEqual([
      'This turn ended without a reply: the runtime finished and wrote nothing back. Nothing was changed. Sending it again usually works.'
    ])

    // A turn that answered says nothing of the kind...
    const answered = buildThread([delta('a', 'ALPHA', 'append', true), event('run.completed', { process: {} })], { running: false })
    expect(answered.some((item) => item.type === 'diagnostic')).toBe(false)
    // ...nor does one that did work without narrating it...
    const worked = buildThread(
      [toolStart('t1', 'shell', 'pnpm test'), toolDone('t1'), event('run.completed', { process: {} })],
      { running: false }
    )
    expect(worked.some((item) => item.type === 'diagnostic')).toBe(false)
    // ...and a run still going has not finished saying anything yet.
    expect(buildThread([event('run.started', {})], { running: true }).some((item) => item.type === 'diagnostic')).toBe(false)
  })

  it('says a quota failure once, not as a card and again as a red line', () => {
    // MEASURED user session 1, 2026-09-05: one Codex quota failure drew the
    // limit card, the runtime's error line and the run's failure card, all
    // carrying "You've hit your usage limit...".
    const said = "You've hit your usage limit. Upgrade to Pro or try again at Sep 7th, 2026 1:57 AM."
    const thread = buildThread(
      [
        toolStart('t1', 'shell', 'pnpm test'),
        event('route.limit_detected', { kind: 'quota-exhausted', message: said }),
        event('adapter.diagnostic', { level: 'error', code: 'codex.runtime_error', message: said, terminal: true }),
        event('adapter.diagnostic', { level: 'warning', code: 'x', message: 'Something else', terminal: false })
      ],
      { running: false }
    )
    expect(thread.filter((i) => i.type === 'limit')).toHaveLength(1)
    expect(thread.filter((i) => i.type === 'diagnostic').map((i) => i.message)).toEqual(['Something else'])
    // The run-level card would be the third copy; a slow-down warning must
    // never hide a real failure reason, so only an ending limit counts.
    expect(errorAlreadyShown(thread, `The run could not continue. ${said}`)).toBe(true)
    expect(errorAlreadyShown(thread, 'The process died')).toBe(false)
    const warned = buildThread([event('route.limit_detected', { kind: 'temporary-rate-limit', message: said })], { running: false })
    expect(errorAlreadyShown(warned, said)).toBe(false)
  })

  it('does not invent an activity card when nothing ran', () => {
    expect(buildThread([delta('a', 'hi', 'append', true)], { running: false }).some((i) => i.type === 'activity'))
      .toBe(false)
  })
})

describe('cancellation summary', () => {
  it('separates tool outcomes and never substitutes them for unreported plan states', () => {
    const summary = cancellationSummary(
      [toolStart('t1', 'shell', 'pnpm build'), toolDone('t1'), toolStart('t2', 'shell', 'pnpm test')],
      4
    )
    expect(summary.settled).toEqual(['pnpm build'])
    expect(summary.interrupted).toEqual(['pnpm test'])
    expect(summary.neverStarted).toBe(4)
  })

  it('counts one file once, however each tool spelled its path', () => {
    // MEASURED 2026-09-07 by drive-stopped-midedit: a stopped run listed five
    // things finished, of which two were the SAME file -- one tool named it
    // absolutely, another relatively -- and one was the workspace directory.
    // The count of what a stopped run finished is the one number on that card
    // a person might act on, and it was inflated by the same file twice.
    const workspace = 'C:' + String.fromCharCode(92) + 'work'
    const absolute = workspace + String.fromCharCode(92) + 'note-1.txt'
    const summary = cancellationSummary(
      [
        toolStart('t1', 'read', absolute),
        toolDone('t1'),
        toolStart('t2', 'write', 'note-1.txt'),
        toolDone('t2')
      ],
      0,
      workspace
    )
    expect(summary.settled).toEqual(['note-1.txt'])
  })

  it('still counts two genuinely different files as two', () => {
    // The negative control. A function that always answered one would satisfy
    // the test above and be useless.
    const workspace = 'C:' + String.fromCharCode(92) + 'work'
    const summary = cancellationSummary(
      [
        toolStart('t1', 'write', 'note-1.txt'),
        toolDone('t1'),
        toolStart('t2', 'write', 'note-2.txt'),
        toolDone('t2')
      ],
      0,
      workspace
    )
    expect(summary.settled).toEqual(['note-1.txt', 'note-2.txt'])
  })

  it('does not repeat a finished item in the interrupted list', () => {
    const summary = cancellationSummary(
      [toolStart('t1', 'write', 'note-1.txt'), toolDone('t1'), toolStart('t2', 'write', 'note-1.txt')],
      0
    )
    expect(summary.settled).toEqual(['note-1.txt'])
    expect(summary.interrupted).toEqual([])
  })

  it('never reports a negative count when more ran than were planned', () => {
    const summary = cancellationSummary([toolStart('t1', 'shell', 'a'), toolDone('t1')], 0)
    expect(summary.neverStarted).toBe(0)
  })
})

describe('signal rail', () => {
  it('shows newest first', () => {
    const rows = buildSignalRail(
      [event('run.started', { runtimeThreadId: 't' }), toolStart('t1', 'shell', 'pnpm test')],
      { running: true }
    )
    expect(rows[0]?.name).toBe('Ran pnpm test')
    expect(rows[1]?.name).toMatch(/^Started on /)
  })

  it('marks a tool live only while it is open AND the run is going', () => {
    const open = [toolStart('t1', 'shell', 'pnpm test')]
    const closed = [toolStart('t1', 'shell', 'pnpm test'), toolDone('t1')]
    expect(buildSignalRail(open, { running: true }).find((r) => r.name.includes('pnpm test'))?.live).toBe(true)
    // The same open tool in a run that has stopped is not live -- a pulsing dot
    // on a dead run is the shell asserting something is happening when nothing
    // is.
    expect(buildSignalRail(open, { running: false })[0]?.live).toBe(false)
    // Settled, it is one row that says so -- not a second "finished" row.
    const settled = buildSignalRail(closed, { running: true }).filter((r) => r.name.includes('pnpm test'))
    expect(settled).toHaveLength(1)
    expect(settled[0]?.live).toBe(false)
    expect(settled[0]?.meta).toMatch(/· done$/)
  })

  it('colours by meaning, not decoration', () => {
    const rows = buildSignalRail(
      [
        event('route.limit_detected', { kind: 'temporary-rate-limit', message: 'slow down' }),
        event('run.failed', { kind: 'process-failed', message: 'died', runtimeTerminal: 'failed', process: {} }),
        event('run.completed', { process: {} })
      ],
      { running: false }
    )
    expect(rows.find((r) => r.name.startsWith('Usage limit'))?.tone).toBe('amber')
    expect(rows.find((r) => r.name.startsWith('Failed'))?.tone).toBe('red')
    expect(rows.find((r) => r.name === 'Finished')?.tone).toBe('blue')
  })

  it('says what happened in words, not the ledger\'s event names', () => {
    // The design review (#10): "Inspector labels in words, not step.started".
    const rows = buildSignalRail(
      [
        event('run.started', { runtimeThreadId: 't' }),
        event('step.started', { stepKind: 'agent_reasoning' }),
        event('plan.updated', { final: true, steps: [] }),
        event('route.limit_detected', { kind: 'temporary-rate-limit', message: 'slow down' }),
        event('run.cancelled', { process: {} })
      ],
      { running: false }
    )
    const names = rows.map((row) => row.name)
    // A step STARTING is bookkeeping; the reasoning step says "Thought" when it ends.
    expect(names).toEqual(['Stopped', 'Usage limit · temporary rate limit', 'Plan updated · final', expect.stringMatching(/^Started on /)])
    for (const name of names) expect(name).not.toMatch(/\b(run|tool|step|route|plan|runtime|adapter)\.[a-z_]+/)
  })

  it('says how far the plan had got, not only that it moved (0.360)', () => {
    // Four rows of "Plan updated" between a run's reads and writes said
    // nothing a person could use (the first-session drive, packaged 0.358).
    const at = (statuses: readonly string[]) =>
      event('plan.updated', { plan: statuses.map((status, index) => ({ id: String(index), content: `step ${String(index)}`, status })) })
    const rows = buildSignalRail([at(['pending', 'pending']), at(['completed', 'in_progress']), at(['completed', 'completed'])], { running: false })
    expect(rows.map((row) => row.name)).toEqual(['Plan: 2 of 2 done', 'Plan: 1 of 2 done', 'Plan: 0 of 2 done'])
  })

  it('says nothing about events it does not understand', () => {
    expect(buildSignalRail([event('nonsense.event', {})], { running: true })).toEqual([])
  })

  /*
   * First-impressions pass, 0.354: the rail read "shell . npm test", "shell
   * finished", "file_change . C:\...", "Step finished . turn" -- the runtimes'
   * type names, two rows a call. It reads as sentences now, one row a call.
   */
  it('says each call as a sentence, one row a call, with how it ended', () => {
    const rows = buildSignalRail(
      [
        event('run.started', { runtimeThreadId: 't' }),
        event('step.started', { stepKind: 'turn' }),
        toolStart('t1', 'shell', 'npm test'),
        event('tool.completed', { itemId: 't1', toolKind: 'command_execution', name: 'shell', phase: 'completed', exitCode: 1 }),
        event('tool.started', { itemId: 't2', toolKind: 'file_change', name: 'file_change', phase: 'started', command: 'C:/work/app/src/signup.ts' }),
        event('tool.completed', { itemId: 't2', toolKind: 'file_change', name: 'file_change', phase: 'completed' }),
        // Reported only when it finished, as OpenCode does: still one row.
        event('tool.completed', { itemId: 't3', toolKind: 'tool', name: 'read', phase: 'completed', command: 'C:/work/app/README.md' }),
        event('step.completed', { stepKind: 'reasoning' }),
        event('step.completed', { stepKind: 'turn' }),
        event('run.completed', { process: {} })
      ],
      { running: false }
    )
    expect(rows.map((row) => row.name)).toEqual(['Finished', 'Thought', 'Read README.md', 'Changed signup.ts', 'Ran npm test', expect.stringMatching(/^Started on /)])
    expect(rows.find((row) => row.name === 'Ran npm test')?.meta).toMatch(/· exit code 1$/)
    expect(rows.find((row) => row.name === 'Changed signup.ts')?.meta).toMatch(/· done$/)
    for (const row of rows) expect(row.name).not.toMatch(/file_change|finished|Step /)
  })

  it('names a call it does not recognise by its own name, never a guess', () => {
    expect(railToolName({ name: 'robinhood.get_quotes', command: 'NVDA' })).toBe('robinhood.get_quotes · NVDA')
    expect(railToolName({ name: 'shell', toolKind: 'command_execution' })).toBe('Ran a command')
    expect(railToolName({ name: 'WebSearch', command: 'oracle force majeure' })).toBe('Searched the web for oracle force majeure')
    // Codex opens a page with the same tool, and its query is then the address.
    expect(railToolName({ name: 'web_search', toolKind: 'web_search', command: 'https://releases.electronjs.org' })).toBe('Opened https://releases.electronjs.org')
  })
})

describe('rail labels', () => {
  it('collapses whitespace and bounds a long command', () => {
    const long = `tool.shell · "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-Content -Raw -LiteralPath .\package.json"`
    const label = railLabel(long)
    expect(label.length).toBeLessThanOrEqual(72)
    expect(label.endsWith('\u2026')).toBe(true)
  })

  it('leaves a short label exactly as it is', () => {
    expect(railLabel('tool.shell · pnpm test')).toBe('tool.shell · pnpm test')
  })

  it('does not let a multi-line command become multiple rail lines', () => {
    expect(railLabel('a\nb\n  c')).toBe('a b c')
  })
})

describe('peer messages in the thread', () => {
  it('hides a share block from the agent bubble, keeping the prose', () => {
    const events = [
      {
        id: 'e1',
        runId: 'run_1',
        missionId: 'mission_1',
        sequence: 1,
        type: 'message.delta',
        occurredAt: '2026-09-01T15:00:00.000Z',
        sourceAdapter: 'codex',
        payload: {
          itemId: 'answer',
          operation: 'replace',
          text: 'The gate is pnpm check.\n\n<locust-share to="Atlas">\npnpm check runs everything.\n</locust-share>',
          final: true,
          evidence: { redacted: true }
        }
      }
    ] as unknown as NormalizedRuntimeEvent[]
    const items = buildThread(events, { running: false })
    expect(items).toEqual([{ key: 'msg_answer', type: 'agent-message', text: 'The gate is pnpm check.', streaming: false }])
  })

  it('groups an exchange by the other party and marks whether anything was received', () => {
    const groups = peerGroups([
      {
        messageId: 'wm_2',
        direction: 'posted',
        from: { teammateId: 'tm_wren', name: 'Wren' },
        to: { teammateId: 'tm_atlas', name: 'Atlas' },
        text: 'Noted as a claim.',
        at: '2026-09-01T15:05:00.000Z'
      },
      {
        messageId: 'wm_1',
        direction: 'received',
        from: { teammateId: 'tm_atlas', name: 'Atlas' },
        to: { teammateId: 'tm_wren', name: 'Wren' },
        text: 'Dispute events changed shape.',
        at: '2026-09-01T15:00:00.000Z'
      },
      {
        messageId: 'wm_3',
        direction: 'posted',
        from: { teammateId: 'tm_wren', name: 'Wren' },
        to: { teammateId: 'tm_nova', name: 'Nova' },
        text: 'Docs are stale.',
        at: '2026-09-01T15:06:00.000Z'
      }
    ])
    expect(groups.map((group) => group.peer.name)).toEqual(['Atlas', 'Nova'])
    expect(groups[0]?.messages.map((message) => message.messageId)).toEqual(['wm_1', 'wm_2'])
    expect(groups[0]?.received).toBe(true)
    expect(groups[1]?.received).toBe(false)
  })
})

describe('reopening a handed-off mission', () => {
  function mission(overrides: Partial<PublicRecoveredMission>): PublicRecoveredMission {
    return {
      missionId: 'mission_1',
      runId: 'run_1',
      workspaceId: 'ws_test',
      prompt: 'Inspect the workspace.',
      runtime: 'codex',
      model: 'account-default',
      requestedRouteId: 'codex',
      resolvedRouteId: 'codex-account:default',
      cliVersion: null,
      createdAt: '2026-09-01T15:00:00.000Z',
      lastUpdatedAt: '2026-09-01T15:00:00.000Z',
      phase: 'completed',
      events: [],
      eventCount: 0,
      eventsTruncated: false,
      integrityIssueCount: 0,
      sandbox: 'read-only',
      checkpoints: [],
      peerMessages: [],
      ...overrides
    }
  }
  const first = mission({
    missionId: 'mission_1',
    runtime: 'codex',
    checkpoints: [
      {
        epoch: 1,
        reason: 'route-switch',
        resumeSafety: 'approval-required',
        safetyReason: 'one action never reported back',
        createdAt: '2026-09-01T15:01:00.000Z',
        unsettledActions: [{ itemId: 'tool_1', name: 'shell' }]
      }
    ]
  })
  const second = mission({
    missionId: 'mission_2',
    runtime: 'claude',
    prompt: 'You are continuing work that another agent (Codex) started...',
    createdAt: '2026-09-01T15:02:00.000Z',
    continuesFrom: { missionId: 'mission_1', checkpointEpoch: 1, reason: 'route-switch' as const }
  })
  const byId = new Map([
    ['mission_1', first],
    ['mission_2', second]
  ])

  it('shows the words the person typed, not the briefing the host wrote', () => {
    expect(rootMission(second, byId).prompt).toBe('Inspect the workspace.')
    expect(rootMission(first, byId)).toBe(first)
  })

  it('rebuilds the divider from the route-switch checkpoint it resumed from', () => {
    const stitched = stitchedHandoff(second, byId)
    expect(stitched).toMatchObject({ from: 'codex', to: 'claude', unsettledCount: 1, omittedBriefing: [] })
    expect(stitchedHandoff(first, byId)).toBeUndefined()
  })

  // 0.519: what the brief left out is recorded, so the amber line survives a reopen.
  it('says what the brief left out, as recorded with the switch', () => {
    const squeezed = mission({ ...second, continuesFrom: { missionId: 'mission_1', checkpointEpoch: 1, reason: 'route-switch' as const, leftOut: ['summary', 'earlier'] } })
    expect(stitchedHandoff(squeezed, new Map([['mission_1', first], ['mission_2', squeezed]]))?.omittedBriefing).toEqual(['summary', 'earlier'])
  })

  it('stops walking a chain whose earlier mission is missing, and a cyclic one', () => {
    const orphan = mission({ missionId: 'mission_3', continuesFrom: { missionId: 'mission_gone', checkpointEpoch: 1, reason: 'route-switch' as const } })
    expect(rootMission(orphan, new Map([['mission_3', orphan]]))).toBe(orphan)
    expect(stitchedHandoff(orphan, new Map([['mission_3', orphan]]))).toBeUndefined()
    const a = mission({ missionId: 'a', continuesFrom: { missionId: 'b', checkpointEpoch: 1, reason: 'route-switch' as const } })
    const b = mission({ missionId: 'b', continuesFrom: { missionId: 'a', checkpointEpoch: 1, reason: 'route-switch' as const } })
    expect(rootMission(a, new Map([['a', a], ['b', b]]))).toBeDefined()
  })
})

describe('the running step', () => {
  function stepEvent(stepKind: 'turn' | 'reasoning' | 'item'): NormalizedRuntimeEvent {
    return {
      id: `e_${stepKind}`,
      runId: 'run_1',
      missionId: 'mission_1',
      sequence: 1,
      type: 'step.started',
      occurredAt: '2026-09-01T15:00:00.000Z',
      sourceAdapter: 'codex',
      payload: { stepKind, evidence: { redacted: true } }
    } as unknown as NormalizedRuntimeEvent
  }

  it('carries the step kind, so thought and action draw differently', () => {
    const thinking = buildThread([stepEvent('reasoning')], { running: true }).find((item) => item.type === 'live-step')
    const acting = buildThread([stepEvent('turn')], { running: true }).find((item) => item.type === 'live-step')
    expect(thinking).toMatchObject({ type: 'live-step', kind: 'reasoning', label: 'Thinking' })
    expect(acting).toMatchObject({ type: 'live-step', kind: 'turn', label: 'Working' })
  })
})

/*
 * Colin, 2026-09-11: thinking, tool calls and connector calls all have to be
 * distinguishable from text the teammate actually wrote. The label could not
 * carry that -- it is whatever the runtime said, so "Exploring the
 * repository" and a sentence of the reply were the same shape of words in the
 * same place. The register is derived from the step's own kind, never from
 * its wording, which is the whole point: a runtime that names its step
 * "Writing the answer" while calling a tool cannot mislabel itself.
 */
describe('which register a live line is', () => {
  const step = (payload: Record<string, unknown>) => event('step.started', payload)

  it('is thinking for a reasoning step, whatever the runtime called it', () => {
    expect(
      buildThread([step({ stepKind: 'reasoning', message: 'Writing the answer' })], { running: true })
        .find((item) => item.type === 'live-step')
    ).toMatchObject({ register: 'thinking' })
  })

  it('is working for a turn', () => {
    expect(
      buildThread([step({ stepKind: 'turn' })], { running: true }).find((item) => item.type === 'live-step')
    ).toMatchObject({ register: 'working' })
  })

  it('is writing for a message item -- the model producing prose, not using anything', () => {
    expect(
      buildThread([step({ stepKind: 'item', itemType: 'assistantMessage' })], { running: true })
        .find((item) => item.type === 'live-step')
    ).toMatchObject({ register: 'writing' })
  })

  it('is tool for any other item, and drops the runtime’s word for the kind', () => {
    // The register names the kind in words a person uses, so printing
    // `mcpToolCall` beside "using a tool" is the same fact twice, in jargon.
    const line = buildThread([step({ stepKind: 'item', itemType: 'mcpToolCall' })], { running: true })
      .find((item) => item.type === 'live-step')
    expect(line).toMatchObject({ register: 'tool' })
    expect(line?.type === 'live-step' ? line.detail : 'set').toBeUndefined()
  })

  it('is the open TOOL, not the turn around it, while one is running', () => {
    // Claude Code reports no step for a tool call, only `tool.started`, so a
    // run that spent thirty seconds reading files said "working" the whole
    // way: honest and useless (MEASURED 2026-09-11, sawATool: false).
    const line = buildThread(
      [
        event('step.started', { stepKind: 'turn' }),
        event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'Read', command: 'README.md', phase: 'started' })
      ],
      { running: true }
    ).find((item) => item.type === 'live-step')
    expect(line).toMatchObject({ register: 'tool', label: 'README.md' })
  })

  it('reads a connector name that carries no mcp__ prefix', () => {
    // MEASURED 2026-09-11, driving a real run: `using a tool ·
    // Google_Drive__create_file`. Every connector name this app had been
    // shown carried the prefix, so the split required one.
    const line = buildThread(
      [event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'Google_Drive__create_file', phase: 'started' })],
      { running: true }
    ).find((item) => item.type === 'live-step')
    expect(line).toMatchObject({ register: 'connector', label: 'create_file', detail: 'Google Drive' })
  })

  it('does not mistake an ordinary tool for a connector', () => {
    // The rule's whole safety is that no ordinary tool carries a double
    // underscore. These are the ones a run actually uses.
    for (const name of ['Read', 'Write', 'Bash', 'Grep', 'Task', 'WebFetch', 'shell', 'apply_patch']) {
      const line = buildThread(
        [event('tool.started', { itemId: 't1', toolKind: 'tool_use', name, phase: 'started' })],
        { running: true }
      ).find((item) => item.type === 'live-step')
      expect(line, name).toMatchObject({ register: 'tool' })
    }
  })

  it('keeps a tool name that contains a double underscore with the server', () => {
    // Non-greedy on purpose: the FIRST `__` after the prefix ends the server,
    // so a tool whose own name carries one stays whole.
    const line = buildThread(
      [event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'mcp__claude_ai_Zapier__run__action', phase: 'started' })],
      { running: true }
    ).find((item) => item.type === 'live-step')
    expect(line).toMatchObject({ register: 'connector', label: 'run__action', detail: 'Zapier' })
  })

  it('names a connector as a connector, because it reaches off this machine', () => {
    const line = buildThread(
      [event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'mcp__claude_ai_Robinhood__get_accounts', phase: 'started' })],
      { running: true }
    ).find((item) => item.type === 'live-step')
    expect(line).toMatchObject({ register: 'connector', label: 'get_accounts', detail: 'Robinhood' })
  })

  it('goes back to the turn once the tool closes', () => {
    const line = buildThread(
      [
        event('step.started', { stepKind: 'turn' }),
        event('tool.started', { itemId: 't1', toolKind: 'tool_use', name: 'Read', command: 'README.md', phase: 'started' }),
        event('tool.completed', { itemId: 't1' })
      ],
      { running: true }
    ).find((item) => item.type === 'live-step')
    expect(line).toMatchObject({ register: 'working' })
  })

  it('is starting before the first event and working after it', () => {
    expect(
      buildThread([], { running: true, startedAt: '2026-09-04T21:47:00.000Z' })
        .find((item) => item.type === 'live-step')
    ).toMatchObject({ register: 'starting' })
    expect(
      buildThread([event('step.started', { stepKind: 'turn' }), event('step.completed', {})], { running: true })
        .find((item) => item.type === 'live-step')
    ).toMatchObject({ register: 'working' })
  })
})

describe('runtime notices in the thread', () => {
  const at = '2026-09-01T15:00:00.000Z'
  function notice(id: string, sequence: number): NormalizedRuntimeEvent {
    return {
      id,
      runId: 'run_1',
      missionId: 'mission_1',
      sequence,
      type: 'adapter.diagnostic',
      occurredAt: at,
      sourceAdapter: 'codex',
      payload: { level: 'warning', code: 'codex.item_error', message: 'Skill descriptions were shortened.', terminal: false, evidence: { redacted: true } }
    } as unknown as NormalizedRuntimeEvent
  }
  const step = {
    id: 's1',
    runId: 'run_1',
    missionId: 'mission_1',
    sequence: 2,
    type: 'step.started',
    occurredAt: at,
    sourceAdapter: 'codex',
    payload: { stepKind: 'turn', evidence: { redacted: true } }
  } as unknown as NormalizedRuntimeEvent

  it('says a notice raised before any work once the turn ends with no fold to put it under (0.586)', () => {
    // It was kept out as setup talk, which left the turn blank when nothing
    // followed it (the cross-runtime output pass, 2026-10-04). Under a fold it
    // still goes under the fold; with none, it is the turn's one line.
    expect(buildThread([notice('d1', 1), step], { running: false }).some((item) => item.type === 'diagnostic')).toBe(true)
    // Codex's real shape: the turn opens, THEN the setup notice arrives, and
    // only after that does anything run. The turn opening is not work.
    expect(buildThread([step, notice('d1', 3)], { running: false }).some((item) => item.type === 'diagnostic')).toBe(true)
    // While the turn runs, the live line stands for it and the notice waits.
    expect(buildThread([step, notice('d1', 3)], { running: true, startedAt: at }).some((item) => item.type === 'diagnostic')).toBe(false)
  })

  it('shows a run in trouble even before any tool has run', () => {
    // MEASURED 2026-09-03: against a dead endpoint Codex retries five times
    // over several minutes, reporting `Reconnecting... 2/5` each time. Those
    // arrive before the first tool, so the gate above dropped every one and
    // the mission sat reading "running" with an empty thread.
    const reconnect = {
      id: 'd9',
      runId: 'run_1',
      missionId: 'mission_1',
      sequence: 1,
      type: 'adapter.diagnostic',
      occurredAt: at,
      sourceAdapter: 'codex',
      payload: {
        level: 'error',
        code: 'codex.runtime_error',
        message: 'Reconnecting... 2/5',
        terminal: false,
        evidence: { redacted: true }
      }
    } as unknown as NormalizedRuntimeEvent
    const items = buildThread([step, reconnect], { running: true })
    expect(items.some((item) => item.type === 'diagnostic' && /Reconnecting/.test(item.message))).toBe(true)
  })

  it('shows a rate-limited OpenCode run retrying, before anything has run', () => {
    // MEASURED 2026-09-25: `opencode run` retrying a 429 said nothing until it
    // gave up; its log line now arrives as this diagnostic (opencode-events).
    // Not a timer -- Colin had the timed quiet line removed on 2026-09-13.
    const retrying = {
      id: 'd10',
      runId: 'run_1',
      missionId: 'mission_1',
      sequence: 1,
      type: 'adapter.diagnostic',
      occurredAt: at,
      sourceAdapter: 'opencode',
      payload: {
        level: 'warning',
        code: 'opencode.runtime_error',
        message: `The model's provider answered "Rate limit exceeded", and OpenCode is trying again on its own.`,
        terminal: false,
        evidence: { redacted: true }
      }
    } as unknown as NormalizedRuntimeEvent
    const items = buildThread([retrying], { running: true })
    expect(items.some((item) => item.type === 'diagnostic' && /trying again on its own/.test(item.message))).toBe(true)
    // And the line under it still says Starting: the provider refusing is not
    // the run working (drive-busy-model, packaged 0.368, read "Working...").
    const started = { ...retrying, id: 'd9', sequence: 0, type: 'run.started', payload: {} } as unknown as NormalizedRuntimeEvent
    expect(buildThread([started, retrying], { running: true, startedAt: at }).find((item) => item.type === 'live-step')).toMatchObject({ label: 'Starting' })
  })

  it('shows a notice raised while the work was under way', () => {
    const items = buildThread([step, toolStart('t1', 'shell', 'pnpm test'), notice('d2', 4)], { running: false })
    expect(items.some((item) => item.type === 'diagnostic')).toBe(true)
  })
})

describe('what a model alias resolved to', () => {
  function ranOn(missionId: string, model: string, resolved: string | undefined, createdAt: string) {
    return {
      missionId,
      runId: `run_${missionId}`,
      workspaceId: 'ws_test',
      prompt: 'x',
      runtime: 'claude' as const,
      model,
      requestedRouteId: 'claude',
      resolvedRouteId: 'claude-account:default',
      cliVersion: null,
      createdAt,
      lastUpdatedAt: createdAt,
      phase: 'completed' as const,
      events: [
        {
          id: `e_${missionId}`,
          runId: `run_${missionId}`,
      workspaceId: 'ws_test',
          missionId,
          sequence: 1,
          type: 'run.started',
          occurredAt: createdAt,
          sourceAdapter: 'claude',
          payload: {
            runtimeThreadId: 'thread',
            evidence: { redacted: true, raw: resolved === undefined ? {} : { model: resolved } }
          }
        }
      ] as unknown as NormalizedRuntimeEvent[],
      eventCount: 1,
      eventsTruncated: false,
      integrityIssueCount: 0,
      sandbox: 'read-only' as const,
      checkpoints: [],
      peerMessages: []
    }
  }

  it('learns the real name the runtime reported for an alias', () => {
    const resolved = resolvedModelNames([ranOn('m1', 'fable', 'claude-fable-5-1', '2026-09-01T10:00:00.000Z')])
    expect(resolved.get('claude:fable')).toBe('claude-fable-5-1')
  })

  it('prefers the newest mission, so a new release replaces an old name', () => {
    // Newest FIRST, which is the order history arrives in. Listed the other
    // way round, plain last-write-wins would land on the right answer by
    // accident and the comparison this pins would not be doing any work.
    const resolved = resolvedModelNames([
      ranOn('new', 'fable', 'claude-fable-5-1', '2026-09-01T10:00:00.000Z'),
      ranOn('old', 'fable', 'claude-fable-5', '2026-08-01T10:00:00.000Z')
    ])
    expect(resolved.get('claude:fable')).toBe('claude-fable-5-1')
  })

  it('says nothing about an alias nobody has run, or one that taught it nothing', () => {
    expect(resolvedModelNames([]).size).toBe(0)
    expect(resolvedModelNames([ranOn('m1', 'fable', undefined, '2026-09-01T10:00:00.000Z')]).size).toBe(0)
    // `fable -> fable` is not a resolution, it is the same word back.
    expect(resolvedModelNames([ranOn('m2', 'fable', 'fable', '2026-09-01T10:00:00.000Z')]).size).toBe(0)
  })
})

describe('a conversation across turns', () => {
  function turn(
    missionId: string,
    prompt: string,
    continuesFrom?: { missionId: string; reason: 'follow-up' | 'route-switch' }
  ): PublicRecoveredMission {
    return {
      missionId,
      runId: `run_${missionId}`,
      workspaceId: 'ws_test',
      prompt,
      runtime: 'claude',
      model: 'sonnet',
      requestedRouteId: 'claude',
      resolvedRouteId: 'claude-account:default',
      cliVersion: null,
      createdAt: '2026-09-02T10:00:00.000Z',
      lastUpdatedAt: '2026-09-02T10:00:00.000Z',
      phase: 'completed',
      events: [],
      eventCount: 0,
      eventsTruncated: false,
      integrityIssueCount: 0,
      sandbox: 'read-only',
      checkpoints: [],
      peerMessages: [],
      ...(continuesFrom === undefined
        ? {}
        : { continuesFrom: { ...continuesFrom, checkpointEpoch: 1 } })
    }
  }

  const first = turn('m1', 'check the google stock price')
  const second = turn('m2', 'cant you look it up for me?', { missionId: 'm1', reason: 'follow-up' })
  const third = turn('m3', 'what about yesterday?', { missionId: 'm2', reason: 'follow-up' })
  const byId = new Map([first, second, third].map((mission) => [mission.missionId, mission] as const))

  it('walks a reply back to every earlier turn, oldest first', () => {
    expect(conversationTurns(third, byId).map((entry) => entry.prompt)).toEqual([
      'check the google stock price',
      'cant you look it up for me?',
      'what about yesterday?'
    ])
  })

  it('is just itself for a first turn', () => {
    expect(conversationTurns(first, byId).map((entry) => entry.missionId)).toEqual(['m1'])
  })

  it('does not walk a route switch, which is a handoff and keeps its divider', () => {
    const handed = turn('m4', 'briefing text', { missionId: 'm1', reason: 'route-switch' })
    const withHandoff = new Map([...byId, ['m4', handed] as const])
    expect(conversationTurns(handed, withHandoff).map((entry) => entry.missionId)).toEqual(['m4'])
  })

  it('stops at a missing or cyclic link rather than spinning', () => {
    const orphan = turn('m9', 'reply', { missionId: 'gone', reason: 'follow-up' })
    expect(conversationTurns(orphan, new Map([['m9', orphan]])).map((e) => e.missionId)).toEqual(['m9'])
    const a = turn('a', 'a', { missionId: 'b', reason: 'follow-up' })
    const b = turn('b', 'b', { missionId: 'a', reason: 'follow-up' })
    const cyclic = new Map([['a', a], ['b', b]] as const)
    expect(conversationTurns(a, cyclic).length).toBeLessThanOrEqual(2)
  })

  it('draws no handoff divider across an ordinary reply', () => {
    expect(stitchedHandoff(second, byId)).toBeUndefined()
  })

  /*
   * A REPLY SENT TO ANOTHER RUNTIME (drive-runtime-switch, packaged 0.309,
   * opened again): the conversation came back as the reply, then the earlier
   * turn's work UNDER it, then the divider -- its first message gone.
   */
  const briefed = (words: string): string =>
    `You are continuing a conversation another agent (Codex) started.\n\n${HANDOFF_INSTRUCTION_MARKER}\n\n${words}`
  const onCodex = { ...turn('s1', 'Remember the word marigold. Reply with just OK.'), runtime: 'codex' as const }
  const switched = {
    ...turn('s2', briefed('What was the word?'), { missionId: 's1', reason: 'route-switch' }),
    runtime: 'opencode' as const,
    createdAt: '2026-09-24T04:13:00.000Z'
  }
  const after = { ...turn('s3', 'And spell it backwards.', { missionId: 's2', reason: 'follow-up' }), runtime: 'opencode' as const }
  const switchedById = new Map([onCodex, switched, after].map((mission) => [mission.missionId, mission] as const))

  it('walks a reply sent to another runtime back to the turns before it, and marks the seam', () => {
    const turns = conversationTurns(switched, switchedById)
    expect(turns.map((entry) => entry.missionId)).toEqual(['s1', 's2'])
    expect(turns[0]?.switchedFrom).toBeUndefined()
    expect(turns[1]?.switchedFrom).toMatchObject({ from: 'codex', to: 'opencode', unsettledCount: 0, omittedBriefing: [] })
    expect(switchOf(switched, switchedById)).toMatchObject({ from: 'codex', to: 'opencode' })
    // The reply's own words, not the host's briefing.
    expect(typedPrompt(switched, switchedById)).toBe('What was the word?')
    // Its seam is drawn before it, not stitched after it with the earlier
    // turn's work under the reply.
    expect(stitchedHandoff(switched, switchedById)).toBeUndefined()
  })

  it('keeps the seam on the switched turn once the conversation moves on', () => {
    const turns = conversationTurns(after, switchedById)
    expect(turns.map((entry) => entry.missionId)).toEqual(['s1', 's2', 's3'])
    expect(turns.map((entry) => entry.switchedFrom?.to)).toEqual([undefined, 'opencode', undefined])
  })

  it('keeps the turns before a running mission that was handed over', () => {
    // A running mission handed over is ONE turn: its work is stitched into
    // the turn (`stitchedHandoff`). The walk used to stop at it, and the turns
    // before it were lost from a reopened conversation.
    const handedMidRun = turn('m5', 'briefing text', { missionId: 'm2', reason: 'route-switch' })
    const withMidRun = new Map([...byId, ['m5', handedMidRun] as const])
    expect(conversationTurns(handedMidRun, withMidRun).map((entry) => entry.missionId)).toEqual(['m1', 'm5'])
    expect(stitchedHandoff(handedMidRun, withMidRun)).toBeDefined()
    expect(switchOf(handedMidRun, withMidRun)).toBeUndefined()
  })
})

describe('whether a finished run can be replied to', () => {
  it('names the session a reply would resume', () => {
    expect(resumableSessionOf([startedEvent('thread-7'), event('run.completed', {})])).toBe('thread-7')
  })

  it('has nothing to resume when the run failed before its runtime started', () => {
    // What a start failure looks like: the host recorded the failure and the
    // runtime never opened a session. A reply here would be refused by the
    // host, so the shell must send it as a new mission instead.
    expect(resumableSessionOf([event('run.failed', { kind: 'process-failed', message: 'Codex CLI is not ready.' })]))
      .toBeUndefined()
    expect(resumableSessionOf([])).toBeUndefined()
  })

  it('ignores an empty session id rather than treating it as one', () => {
    expect(resumableSessionOf([startedEvent()])).toBeUndefined()
  })
})

describe('which words a turn shows', () => {
  const mission = (
    missionId: string,
    prompt: string,
    continuesFrom?: { readonly missionId: string; readonly reason: 'route-switch' | 'follow-up' }
  ): PublicRecoveredMission => ({
    missionId,
    runId: `run_${missionId}`,
      workspaceId: 'ws_test',
    prompt,
    runtime: 'codex',
    model: 'account-default',
    resolvedRouteId: 'codex-account:default',
    cliVersion: null,
    sandbox: 'read-only',
    phase: 'completed',
    createdAt: NOW,
    lastUpdatedAt: NOW,
    integrityIssueCount: 0,
    events: [],
    peerMessages: [],
    ...(continuesFrom === undefined
      ? {}
      : { continuesFrom: { ...continuesFrom, checkpointEpoch: 1 } })
  } as unknown as PublicRecoveredMission)

  const index = (missions: readonly PublicRecoveredMission[]) =>
    new Map(missions.map((held) => [held.missionId, held]))

  it('shows a reply the words that were typed for it, not the opening line', () => {
    const first = mission('m1', 'Audit the config')
    const reply = mission('m2', 'Now fix the two you found', { missionId: 'm1', reason: 'follow-up' })
    expect(typedPrompt(reply, index([first, reply]))).toBe('Now fix the two you found')
  })

  it('shows a handed-over mission the words a person typed, not the briefing written for it', () => {
    const first = mission('m1', 'Audit the config')
    const handed = mission('m2', 'You are continuing a mission…', { missionId: 'm1', reason: 'route-switch' })
    expect(typedPrompt(handed, index([first, handed]))).toBe('Audit the config')
  })

  it('reaches back through a handoff but stops at the reply above it', () => {
    // A → handed over → B → replied to → C. C's own words are C's.
    const a = mission('m1', 'Audit the config')
    const b = mission('m2', 'You are continuing a mission…', { missionId: 'm1', reason: 'route-switch' })
    const c = mission('m3', 'Now fix the two you found', { missionId: 'm2', reason: 'follow-up' })
    const byId = index([a, b, c])
    expect(typedPrompt(c, byId)).toBe('Now fix the two you found')
    expect(typedPrompt(b, byId)).toBe('Audit the config')
  })

  it('keeps the mission own words when the one it continues is gone', () => {
    const orphan = mission('m2', 'You are continuing a mission…', { missionId: 'm_missing', reason: 'route-switch' })
    expect(typedPrompt(orphan, index([orphan]))).toBe('You are continuing a mission…')
  })
})

describe('the routes this person has actually run', () => {
  const ran = (missionId: string, runtime: string, model: string, lastUpdatedAt: string) => ({
    missionId,
    runId: `run_${missionId}`,
      workspaceId: 'ws_test',
    prompt: 'x',
    runtime,
    model,
    resolvedRouteId: `${runtime}-account:default`,
    cliVersion: null,
    sandbox: 'read-only',
    phase: 'completed',
    createdAt: lastUpdatedAt,
    lastUpdatedAt,
    integrityIssueCount: 0,
    events: [],
    peerMessages: []
  } as unknown as PublicRecoveredMission)

  it('lists each route once, newest first', () => {
    const routes = recentlyUsedRoutes([
      ran('m1', 'codex', 'gpt-5', '2026-09-01T00:00:00.000Z'),
      ran('m2', 'cursor', 'composer-2.5', '2026-09-02T00:00:00.000Z'),
      ran('m3', 'codex', 'gpt-5', '2026-09-03T00:00:00.000Z')
    ])
    expect(routes).toEqual(['codex:gpt-5', 'cursor:composer-2.5'])
  })

  it('says nothing when nothing has been run', () => {
    expect(recentlyUsedRoutes([])).toEqual([])
  })
})

describe('the activity card reads the change, not a receipt of it', () => {
  const PATCH = [
    '--- a/src/billing.ts',
    '+++ b/src/billing.ts',
    '@@ -12,2 +12,3 @@ handle()',
    ' const a = 1;',
    '-const b = 2;',
    '+const b = 3;',
    '+const c = 4;',
    ''
  ].join('\n')

  function edited(patch: { text: string; added: number; removed: number; truncated: boolean }) {
    return [
      { kind: 'edit', name: 'apply_patch', settled: true, patch }
    ]
  }

  it('turns one patch into a row per file it touched', () => {
    const two = `${PATCH}--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n`
    const entries = activityEntries(edited({ text: two, added: 3, removed: 2, truncated: false }))
    expect(entries.map((entry) => (entry.kind === 'file' ? entry.file.path : entry.kind))).toEqual([
      'src/billing.ts',
      'README.md'
    ])
  })

  it('sums the card total from the rows it will actually draw', () => {
    // The runtime's own header claims far more than the recorded text holds.
    // The card shows what the diff below it can show, or the two disagree.
    const entries = activityEntries(edited({ text: PATCH, added: 900, removed: 900, truncated: true }))
    expect(activityCounts(edited({ text: PATCH, added: 900, removed: 900, truncated: true }))).toEqual({
      added: 2,
      removed: 1
    })
    expect(entries[0]?.kind === 'file' ? entries[0].counts : undefined).toEqual({ added: 2, removed: 1 })
  })

  it('keeps the runtime total beside a truncated single-file patch, and withholds it across several', () => {
    const one = activityEntries(edited({ text: PATCH, added: 900, removed: 900, truncated: true }))[0]
    expect(one?.kind === 'file' ? one.reported : undefined).toEqual({ added: 900, removed: 900 })
    const many = activityEntries(
      edited({ text: `${PATCH}--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n`, added: 900, removed: 900, truncated: true })
    )[0]
    expect(many?.kind === 'file' ? many.reported : 'missing').toBeUndefined()
  })

  it('keeps an edit whose runtime reported no patch, as a row that says so', () => {
    const entries = activityEntries([{ kind: 'edit', name: 'apply_patch', settled: true }])
    expect(entries).toHaveLength(1)
    expect(entries[0]?.kind).toBe('unreported')
  })

  it('says an edit whose result was too large to keep, rather than that nobody confirmed it', () => {
    const entries = activityEntries([{ kind: 'edit', name: 'src/app.ts', tool: 'edit', settled: true, status: 'result too large to keep' }])
    expect(entries[0]).toMatchObject({ kind: 'unreported', name: 'src/app.ts', settled: true, tooLarge: true })
  })

  it('names the tool on a row whose target is the same path it edited', () => {
    // One call each, because two consecutive plain tool calls now FOLD into a
    // single row (`foldPlainToolRuns`). The naming rule this protects is
    // about how a row is built, so it is checked on rows that stayed rows.
    const read = activityEntries([{ kind: 'tool', name: 'src/billing.ts', tool: 'read', settled: true }])
    expect(read[0]).toMatchObject({ kind: 'tool', name: 'src/billing.ts', tool: 'read' })
    // A tool whose name IS its target says it once, not twice.
    const grep = activityEntries([{ kind: 'tool', name: 'grep', tool: 'grep', settled: true }])
    expect(grep[0]).toMatchObject({ tool: undefined })
  })

  it('folds a run of plain tool calls, and never the command beside them', () => {
    const entries = activityEntries([
      { kind: 'tool', name: 'src/billing.ts', tool: 'read', settled: true },
      { kind: 'tool', name: 'src/tax.ts', tool: 'read', settled: true },
      { kind: 'shell', name: 'pnpm test', settled: true }
    ])
    expect(entries.map((entry) => entry.kind)).toEqual(['tools', 'shell'])
  })

  it('carries a command row with its exit result', () => {
    const entries = activityEntries([
      { kind: 'shell', name: 'pnpm test', settled: true, failed: true, exitCode: 1 }
    ])
    expect(entries[0]).toMatchObject({ kind: 'shell', command: 'pnpm test', failed: true, exitCode: 1 })
  })

  it('opens the first file, unless opening it would bury everything after it', () => {
    const small = activityEntries(edited({ text: PATCH, added: 2, removed: 1, truncated: false }))
    expect(defaultOpenEntry(small)).toBe(small[0]?.key)
    const huge = [
      '--- a/big.ts',
      '+++ b/big.ts',
      `@@ -1 +1,400 @@`,
      ...Array.from({ length: 400 }, (_, i) => `+line ${String(i)}`)
    ].join('\n')
    const big = activityEntries(edited({ text: huge, added: 400, removed: 0, truncated: false }))
    expect(big[0]?.kind === 'file' ? big[0].large : false).toBe(true)
    expect(defaultOpenEntry(big)).toBeUndefined()
  })

  it('opens a new web page however long it is, because it is shown running (0.446)', () => {
    // drive-compare-pages: the column that built the longer page showed a
    // folded "index.html ADDED LARGE" row beside its rival's running page.
    const page = [
      '--- /dev/null',
      '+++ b/index.html',
      `@@ -0,0 +1,400 @@`,
      ...Array.from({ length: 400 }, (_, i) => `+<p>line ${String(i)}</p>`)
    ].join('\n')
    const built = activityEntries(edited({ text: page, added: 400, removed: 0, truncated: false }))
    expect(built[0]?.kind === 'file' ? [built[0].large, built[0].file.status] : undefined).toEqual([true, 'ADDED'])
    expect(defaultOpenEntry(built)).toBe(built[0]?.key)
  })

  it('attaches a completion patch to the tool that opened, and names the runtime that reported it', () => {
    const thread = buildThread(
      [
        event('tool.started', { itemId: 't1', toolKind: 'file_change', name: 'apply_patch', phase: 'started' }),
        event('tool.completed', {
          itemId: 't1',
          toolKind: 'file_change',
          name: 'apply_patch',
          phase: 'completed',
          patch: { text: PATCH, added: 2, removed: 1, truncated: false }
        })
      ],
      { running: false }
    )
    const card = thread.find((item) => item.type === 'activity')
    expect(card?.type === 'activity' ? card.reportedBy : undefined).toBe('codex')
    expect(card?.type === 'activity' ? activityCounts(card.details) : undefined).toEqual({ added: 2, removed: 1 })
  })
})

describe('marking time in a long conversation', () => {
  function turn(minute: number, count = 1) {
    return Array.from({ length: count }, (_, index) =>
      event('step.completed', { itemId: `s${String(minute)}_${String(index)}`, stepKind: 'turn' })
    ).map((each, index) => ({
      ...each,
      occurredAt: new Date(Date.UTC(2026, 7, 31, 16, minute + index)).toISOString()
    })) as unknown as import('@teammate/runtime-adapters').NormalizedRuntimeEvent[]
  }

  it('says nothing when turns follow each other closely', () => {
    expect(threadMarkers([turn(0), turn(1)])).toEqual([])
  })

  it('marks a gap, with the elapsed mission time and how long the wait was', () => {
    const markers = threadMarkers([turn(0), turn(40)])
    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({ beforeTurn: 1, minutesIn: 40, note: 'waited 40 min' })
  })

  it('measures elapsed time from the mission first event, not the previous turn', () => {
    const markers = threadMarkers([turn(0), turn(10), turn(30)])
    expect(markers.map((marker) => marker.minutesIn)).toEqual([10, 30])
  })
})

describe('both halves of a teammate exchange are drawn', () => {
  const message = (
    direction: 'received' | 'posted',
    who: string,
    text: string,
    at: string
  ): PublicPeerMessage => ({
    messageId: `m_${text.slice(0, 6)}_${direction}`,
    direction,
    from: direction === 'posted' ? { teammateId: 'booty', name: 'Booty' } : { teammateId: 'wren', name: 'Wren' },
    to: direction === 'posted' ? { teammateId: 'wren', name: who } : { teammateId: 'booty', name: 'Booty' },
    text,
    at
  })

  // Turn 1: Booty asks Wren. Turn 2: Wren's answer comes back.
  const turns = [
    [message('posted', 'Wren', 'Write me a soliloquy.', '2026-09-04T21:47:00.000Z')],
    [message('received', 'Booty', 'O silent hall of half-built code...', '2026-09-04T21:48:00.000Z')]
  ]

  it('draws the message an earlier turn SENT, not only the reply', () => {
    // The bug Colin caught: the thread showed Wren's soliloquy and never
    // showed Booty asking for it, so it read as though Wren answered him.
    const cards = threadPeerCards(turns)
    const sent = cards.flatMap((card) => card.group.messages).filter((held) => held.direction === 'posted')
    expect(sent.map((held) => held.text)).toEqual(['Write me a soliloquy.'])
  })

  it('keeps every turn of a long conversation, not just the last two', () => {
    // Guards the shape of the fix: a version that kept only the newest turns
    // would pass the test above and still lose the start of the exchange.
    const many = Array.from({ length: 6 }, (_unused, index) =>
      [message('posted', 'Wren', `ask ${String(index)}`, `2026-09-04T21:${String(40 + index)}:00.000Z`)]
    )
    const texts = threadPeerCards(many).flatMap((card) => card.group.messages).map((held) => held.text)
    expect(texts).toEqual(['ask 0', 'ask 1', 'ask 2', 'ask 3', 'ask 4', 'ask 5'])
  })

  it('files each card against the turn it happened on', () => {
    const cards = threadPeerCards(turns)
    expect(cards.map((card) => card.turnIndex)).toEqual([0, 1])
  })

  it('puts what a turn sent after its work and what it was handed before it', () => {
    const cards = threadPeerCards(turns)
    expect(cards[0]?.placement).toBe('after-work')
    expect(cards[1]?.placement).toBe('before-work')
  })

  it('gives the same peer a distinct key on each turn', () => {
    // One key per peer would collapse two turns of an exchange into one card.
    const keys = threadPeerCards(turns).map((card) => card.key)
    expect(new Set(keys).size).toBe(keys.length)
  })
})

describe('reading an exchange without hunting for it', () => {
  it('opens a short exchange in place', () => {
    // An ask and an answer is the common case and the one that went unread.
    expect(peerExchangeStartsOpen(1)).toBe(true)
    expect(peerExchangeStartsOpen(2)).toBe(true)
  })

  it('leaves a long back-and-forth collapsed', () => {
    // The reason the card collapses at all: a colleague's long aside must not
    // read as the mission's own work.
    expect(peerExchangeStartsOpen(3)).toBe(false)
    expect(peerExchangeStartsOpen(9)).toBe(false)
  })

  it('has nothing to open when there are no messages', () => {
    expect(peerExchangeStartsOpen(0)).toBe(false)
  })

  it('previews what a collapsed exchange said', () => {
    expect(peerSnippet('Write me a soliloquy.')).toBe('Write me a soliloquy.')
  })

  it('flattens a multi-line message to one line', () => {
    expect(peerSnippet('first line\n\n  second line')).toBe('first line second line')
  })

  it('cuts a long message rather than letting the card wrap', () => {
    const snippet = peerSnippet('x'.repeat(200), 20)
    expect(snippet).toHaveLength(20)
    expect(snippet?.endsWith('…')).toBe(true)
  })

  it('previews nothing for a message the workroom no longer holds', () => {
    expect(peerSnippet(null)).toBeUndefined()
    expect(peerSnippet('   ')).toBeUndefined()
  })
})

describe('a run the host started for a teammate', () => {
  const relayed = (over: Partial<PublicRecoveredMission> = {}): PublicRecoveredMission =>
    ({
      missionId: 'm_relay',
      runId: 'run_relay',
      workspaceId: 'ws_test',
      runtime: 'claude',
      model: 'sonnet',
      resolvedRouteId: 'claude:sonnet',
      cliVersion: null,
      sandbox: 'read-only',
      phase: 'completed',
      createdAt: NOW,
      lastUpdatedAt: NOW,
      integrityIssueCount: 0,
      events: [],
      prompt:
        'Wren (Code & Migrations) sent you a message; it is quoted below with anything else waiting for you. Do what it asks if that is within your role and this workspace. Write back only if that helps finish the work: end with one <locust-share to="Wren"> block holding your reply.',
      startedBy: { kind: 'relay', hop: 1 },
      peerMessages: [
        {
          messageId: 'msg_1',
          direction: 'received',
          from: { teammateId: 'tm_wren', name: 'Wren' },
          to: { teammateId: 'tm_booty', name: 'Booty' },
          text: 'Please reply with the passphrase\n  PEBBLE-9993.',
          at: '2026-09-04T21:47:00.000Z'
        }
      ],
      ...over
    }) as PublicRecoveredMission

  it('is named by the message that caused it, never by the host briefing', () => {
    // The briefing is instructions to a runtime. It was appearing as the NAME
    // of a mission beside conversations a person actually started.
    expect(relayedTitle(relayed())).toBe('Wren asked: Please reply with the passphrase PEBBLE-9993.')
  })

  it('leaves a mission a person started alone', () => {
    expect(relayedTitle({ ...relayed(), startedBy: undefined } as PublicRecoveredMission)).toBeUndefined()
  })

  it('shows the briefing rather than inventing a title when the message is gone', () => {
    // A message the workroom no longer holds reads as null. The briefing is at
    // least true; a made-up title is not.
    const gone = relayed({
      peerMessages: [{ ...relayed().peerMessages[0]!, text: null }]
    })
    expect(relayedTitle(gone)).toBeUndefined()
  })

  it('ignores what the run SENT and names it by what it was asked', () => {
    const sentOnly = relayed({
      peerMessages: [{ ...relayed().peerMessages[0]!, direction: 'posted' }]
    })
    expect(relayedTitle(sentOnly)).toBeUndefined()
  })

  it('keeps the title to one line', () => {
    expect(relayedTitle(relayed())).not.toContain('\n')
  })

  it('does not let typedPrompt hand back the briefing either', () => {
    const title = typedPrompt(relayed(), new Map())
    expect(title).toBe('Wren asked: Please reply with the passphrase PEBBLE-9993.')
    expect(title).not.toContain('locust-share')
  })
})

describe('a question the run ended on', () => {
  const ASK = [
    'Here is what I found.',
    '',
    '<locust-ask>',
    'Keep the two callers on v2, or migrate them now?',
    '- Keep them on v2 :: Smaller change',
    '- Migrate all callers now :: Touches 4 more files',
    '</locust-ask>'
  ].join('\n')

  const finished = (text: string): readonly NormalizedRuntimeEvent[] => [delta('a', text, 'append', true)]

  it('becomes a card on the turn a person can answer', () => {
    const items = buildThread(finished(ASK), { running: false, latestTurn: true })
    const card = items.find((item) => item.type === 'decision')
    expect(card?.type === 'decision' && card.request.options).toHaveLength(2)
  })

  it('is not offered on an earlier turn, where the answer already exists', () => {
    // On an earlier turn the answer IS the next turn's prompt, a few lines
    // below. Offering buttons there invites answering the same fork twice.
    expect(buildThread(finished(ASK), { running: false }).some((i) => i.type === 'decision')).toBe(false)
  })

  it('is not offered while the run is still going', () => {
    // A block still streaming may not have its closing tag yet, and a run that
    // has not stopped has not asked.
    expect(buildThread(finished(ASK), { running: true, latestTurn: true }).some((i) => i.type === 'decision')).toBe(false)
  })

  it('is taken out of the reply bubble, so it is never asked twice', () => {
    const items = buildThread(finished(ASK), { running: false, latestTurn: true })
    const message = items.find((item) => item.type === 'agent-message')
    expect(message?.type === 'agent-message' && message.text).toBe('Here is what I found.')
  })

  it('leaves an ordinary finished run with no card', () => {
    expect(buildThread(finished('Done, two files changed.'), { running: false, latestTurn: true })
      .some((i) => i.type === 'decision')).toBe(false)
  })

  it('reads the LAST final message, not an earlier one', () => {
    // A run can answer, then ask. The question it stopped on is the last one.
    const items = buildThread(
      [delta('a', 'First pass done.', 'append', true), delta('b', ASK, 'append', true)],
      { running: false, latestTurn: true }
    )
    expect(items.some((i) => i.type === 'decision')).toBe(true)
  })
})

describe('what the decision card may say about the workspace', () => {
  it('claims nothing changed only when the run could not write', () => {
    // A guarantee from the sandbox, not an observation.
    expect(decisionStanding({ sandbox: 'read-only', events: [] })).toContain('nothing was changed')
  })

  it('says work is kept when a patch actually came back', () => {
    const wrote = [event('tool.completed', { itemId: 't1', toolKind: 'edit', name: 'apply', phase: 'completed', patch: 'diff' })]
    expect(decisionStanding({ sandbox: 'workspace-write', events: wrote })).toContain('work already done is kept')
  })

  it('never claims nothing changed for a run that was allowed to write', () => {
    // Nothing was OBSERVED, which is not the same as nothing happening: a
    // runtime need not report every write. So it states the permission.
    const said = decisionStanding({ sandbox: 'workspace-write', events: [] })
    expect(said).not.toContain('nothing was changed')
    expect(said).toContain('could edit files')
  })

  it('does not claim nothing changed when the mode is unknown', () => {
    expect(decisionStanding({ sandbox: undefined, events: [] })).not.toContain('nothing was changed')
  })
})

describe('the waiting line’s clock', () => {
  it('counts from the start of the turn, so it climbs instead of looping', () => {
    // Colin watched one count to 10 and start over, repeatedly, which reads as
    // a stuck loop rather than a run making progress. Clocking from the last
    // event did that: a runtime reporting every few seconds reset it every few
    // seconds.
    const items = buildThread(
      [
        event('run.started', {}),
        event('adapter.diagnostic', { level: 'info', message: 'something later' })
      ],
      { running: true, latestTurn: true, startedAt: '2026-09-05T10:00:00.000Z' }
    )
    const line = items.find((item) => item.type === 'live-step')
    expect(line?.type === 'live-step' && line.startedAt).toBe('2026-09-05T10:00:00.000Z')
  })

  it('falls back to the first event when the turn start is unknown', () => {
    // A restored mission has no send time in hand; the first event is still
    // the earliest moment the app can honestly count from.
    const items = buildThread([event('run.started', {})], { running: true, latestTurn: true })
    const line = items.find((item) => item.type === 'live-step')
    expect(line?.type === 'live-step' && line.startedAt).toBe(NOW)
  })
})

describe("the host's disk observation of a path the runtime named", () => {
  it('attaches the patch to the row the runtime drew, and draws a row only for a path it never named', () => {
    const patch = { text: '--- /dev/null\n+++ b/NOTES.md\n@@ -0,0 +1,2 @@\n+# Notes\n+First entry.\n', added: 2, removed: 0, truncated: false }
    const thread = buildThread(
      [
        event('tool.started', { itemId: 'fc', toolKind: 'file_change', name: 'file_change', command: 'NOTES.md', phase: 'started' }),
        event('tool.completed', { itemId: 'fc', toolKind: 'file_change', name: 'file_change', command: 'NOTES.md', phase: 'completed' }),
        event('tool.started', { itemId: 'disk-observed-1', toolKind: 'observed_edit', name: 'edit', command: 'NOTES.md', status: 'reported by the runtime, read from disk', phase: 'started' }),
        event('tool.completed', { itemId: 'disk-observed-1', toolKind: 'observed_edit', name: 'edit', command: 'NOTES.md', status: 'reported by the runtime, read from disk', phase: 'completed', patch }),
        event('tool.started', { itemId: 'disk-observed-2', toolKind: 'observed_edit', name: 'edit', command: 'other.txt', status: 'observed on disk', phase: 'started' }),
        event('tool.completed', { itemId: 'disk-observed-2', toolKind: 'observed_edit', name: 'edit', command: 'other.txt', status: 'observed on disk', phase: 'completed' })
      ],
      { running: false }
    )
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    const notes = details.filter((detail) => /NOTES/.test(detail.name))
    expect(notes).toHaveLength(1)
    expect(notes[0]?.patch?.added).toBe(2)
    expect(details.some((detail) => /other\.txt/.test(detail.name))).toBe(true)
    expect(activity?.type === 'activity' && activity.summary).toBe('Edited 2 files')
  })

  it('still attaches when the runtime already sent its OWN diff for that path', () => {
    // MEASURED 2026-09-07 by `_tools/drive-reveal.mjs`, Cursor Agent writing
    // one file: the fold drew `report.md ADDED +1 -0` TWICE while the line
    // above it said `1 file`. The merge above required the runtime's row to
    // have NO patch -- which is true of Codex, whose file_change names a path
    // and sends no diff, and false of Cursor, which sends one. So for every
    // runtime that diffs its own edits, the host's disk observation of the
    // same change became a second identical row.
    const own = { text: '--- /dev/null\n+++ b/report.md\n@@ -0,0 +1 @@\n+hello\n', added: 1, removed: 0, truncated: false }
    const observed = { text: '--- /dev/null\n+++ b/report.md\n@@ -0,0 +1 @@\n+hello\n', added: 1, removed: 0, truncated: false }
    const thread = buildThread(
      [
        event('tool.started', { itemId: 'w', toolKind: 'write', name: 'write', command: 'report.md', phase: 'started' }),
        event('tool.completed', { itemId: 'w', toolKind: 'write', name: 'write', command: 'report.md', phase: 'completed', patch: own }),
        event('tool.started', { itemId: 'obs', toolKind: 'observed_edit', name: 'edit', command: 'report.md', status: 'reported by the runtime, read from disk', phase: 'started' }),
        event('tool.completed', { itemId: 'obs', toolKind: 'observed_edit', name: 'edit', command: 'report.md', status: 'reported by the runtime, read from disk', phase: 'completed', patch: observed })
      ],
      { running: false }
    )
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    // Since 0.494 the step keeps its own row and the host's look is its own;
    // the turn's files card shows the file once, as its net change.
    expect(netFileEntries(activityEntries(details)).filter((entry) => entry.kind === 'file' && /report\.md/.test(entry.file.path))).toHaveLength(1)
    expect(activity?.type === 'activity' && activity.summary).toBe('Edited 1 file')
  })

  it('supersedes every other edit row for that file, so the card counts the file once', () => {
    // MEASURED 2026-09-16 by Astra on 0.154.0 (mission_2e14e822): OpenCode
    // changed two lines of one README in two edits, +1 -1 each. The host's
    // observation (+2 -2) attached to the FIRST row and the second stayed,
    // so the card said `1 file +3 -3` over a file git reported as `2 2`.
    const first = { text: '--- a/README.md\n+++ b/README.md\n@@ -3 +3 @@\n-Colour: blue\n+Colour: green\n', added: 1, removed: 1, truncated: false }
    const second = { text: '--- a/README.md\n+++ b/README.md\n@@ -4 +4 @@\n-Status: draft\n+Status: ready\n', added: 1, removed: 1, truncated: false }
    const observed = { text: '--- a/README.md\n+++ b/README.md\n@@ -3,2 +3,2 @@\n-Colour: blue\n-Status: draft\n+Colour: green\n+Status: ready\n', added: 2, removed: 2, truncated: false }
    const absolute = 'C:\\work\\README.md'
    const thread = buildThread(
      [
        event('tool.started', { itemId: 'e1', toolKind: 'edit', name: 'edit', command: absolute, phase: 'started' }),
        event('tool.completed', { itemId: 'e1', toolKind: 'edit', name: 'edit', command: absolute, phase: 'completed', patch: first }),
        event('tool.started', { itemId: 'e2', toolKind: 'edit', name: 'edit', command: absolute, phase: 'started' }),
        event('tool.completed', { itemId: 'e2', toolKind: 'edit', name: 'edit', command: absolute, phase: 'completed', patch: second }),
        event('tool.started', { itemId: 'obs', toolKind: 'observed_edit', name: 'edit', command: 'README.md', status: 'reported by the runtime, read from disk', phase: 'started' }),
        event('tool.completed', { itemId: 'obs', toolKind: 'observed_edit', name: 'edit', command: 'README.md', status: 'reported by the runtime, read from disk', phase: 'completed', patch: observed })
      ],
      { running: false }
    )
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    // The card (0.494): one row for the file, its net change, counted once.
    const card = netFileEntries(activityEntries(details, 'C:\\work'), 'C:\\work').filter((entry) => entry.kind === 'file')
    expect(card).toHaveLength(1)
    expect(card[0]?.kind === 'file' ? card[0].counts : undefined).toEqual({ added: 2, removed: 2 })
    expect(activity?.type === 'activity' && activity.summary).toBe('Edited 1 file')
  })

  it('leaves two edits to one file alone when no observation arrives', () => {
    // Without the disk's word, each runtime row is the only evidence there
    // is, and both are drawn and counted -- as the fold has always done.
    const first = { text: '--- a/README.md\n+++ b/README.md\n@@ -3 +3 @@\n-Colour: blue\n+Colour: green\n', added: 1, removed: 1, truncated: false }
    const second = { text: '--- a/README.md\n+++ b/README.md\n@@ -4 +4 @@\n-Status: draft\n+Status: ready\n', added: 1, removed: 1, truncated: false }
    const thread = buildThread(
      [
        event('tool.started', { itemId: 'e1', toolKind: 'edit', name: 'edit', command: 'README.md', phase: 'started' }),
        event('tool.completed', { itemId: 'e1', toolKind: 'edit', name: 'edit', command: 'README.md', phase: 'completed', patch: first }),
        event('tool.started', { itemId: 'e2', toolKind: 'edit', name: 'edit', command: 'README.md', phase: 'started' }),
        event('tool.completed', { itemId: 'e2', toolKind: 'edit', name: 'edit', command: 'README.md', phase: 'completed', patch: second })
      ],
      { running: false }
    )
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    expect(activityCounts(details)).toEqual({ added: 2, removed: 2 })
  })
})

describe('a usage window, as a person reads it', () => {
  it('turns ISO reset instants into clock times and leaves the words alone', () => {
    const label = usageWindowLabel('5-hour window 67% used · resets 2026-09-06T02:10:00.000Z · 7-day window 53% used')
    expect(label).not.toContain('2026-09-06T')
    expect(label).toMatch(/^5-hour window 67% used · resets .+ · 7-day window 53% used$/)
    expect(usageWindowLabel('nothing to convert')).toBe('nothing to convert')
  })
})

describe('the trace line for a finished turn (SURFACES-0.22)', () => {
  const at = (s: number) => new Date(1_700_000_000_000 + s * 1000).toISOString()
  const base = { runId: 'run_1', missionId: 'mission_1', sourceAdapter: 'claude' as const }
  const ev = (seq: number, type: string, payload: Record<string, unknown>, s: number) => ({ ...base, id: `e${String(seq)}`, sequence: seq, occurredAt: at(s), type, payload: { evidence: { redacted: true }, ...payload } }) as never
  const joined = (segments: readonly { text: string }[]) => segments.map((seg) => seg.text).join(' · ')

  it('leads with the duration and ends with the exceptions; a zero segment is absent', () => {
    const events = [
      ev(1, 'run.started', { runtimeThreadId: 't' }, 0),
      ev(2, 'step.started', { stepKind: 'reasoning', itemId: 'r1' }, 1),
      ev(3, 'step.completed', { stepKind: 'reasoning', itemId: 'r1' }, 8),
      ev(4, 'tool.started', { itemId: 'a', toolKind: 'tool_use', name: 'Agent', command: 'Count lines', phase: 'started' }, 9),
      ev(5, 'tool.completed', { itemId: 'a', toolKind: 'tool_use', name: 'Agent', command: 'Count lines', phase: 'completed', status: 'Explore', output: '3' }, 20),
      ev(6, 'tool.started', { itemId: 'b', toolKind: 'tool_use', name: 'Read', command: 'README.md', phase: 'started' }, 21),
      ev(7, 'tool.completed', { itemId: 'b', toolKind: 'tool_use', name: 'Read', command: 'README.md', phase: 'completed' }, 22),
      ev(8, 'adapter.diagnostic', { code: 'claude.notification', level: 'warning', terminal: false, message: 'Stop hook error occurred' }, 40),
      ev(9, 'run.completed', { runtimeThreadId: 't', process: {} }, 41)
    ]
    const thread = buildThread(events, { running: false })
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    const segments = activityTrace(details, events, traceOutcome(events, false))
    /*
     * No `1 notice` on the end any more.
     *
     * The chip counted a thing the person could not read, and counted a
     * DIFFERENT set than the thread drew -- this function took every
     * diagnostic bar a usage window, while the thread drops any that arrives
     * before the first tool call unless it is a run-level error. A real
     * `seq 1 300` capture showed `1 notice` with no sentence anywhere on the
     * screen. The notice itself is now drawn at the foot of the fold instead;
     * `carries the notices it used to only count` below is the other half.
     */
    expect(joined(segments)).toBe('41s · thought 7s · asked 1 subagent · 1 tool call')
    expect(segments.some((seg) => seg.key === 'notices')).toBe(false)
    expect(segments.find((seg) => seg.key === 'subagents')?.tone).toBeUndefined()
  })

  it('counts the commands first, and calls the rest other tool calls', () => {
    // Yurt's fat turn (beta report, #6): "3 tool calls · ran mkdir, printf and
    // 10 more" over twelve command rows read as three calls in all. The three
    // were reads; the commands were twelve.
    const shell = (at: number, id: string, command: string) => [
      ev(at, 'tool.started', { itemId: id, toolKind: 'command_execution', name: 'bash', command, phase: 'started' }, at),
      ev(at + 1, 'tool.completed', { itemId: id, toolKind: 'command_execution', name: 'bash', command, phase: 'completed', exitCode: 0 }, at + 1)
    ]
    const read = (at: number, id: string, path: string) => [
      ev(at, 'tool.started', { itemId: id, toolKind: 'read', name: 'read', command: path, phase: 'started' }, at),
      ev(at + 1, 'tool.completed', { itemId: id, toolKind: 'read', name: 'read', command: path, phase: 'completed' }, at + 1)
    ]
    const events = [
      ev(1, 'run.started', { runtimeThreadId: 't' }, 0),
      ...read(2, 'r1', 'LOCUST.md'), ...read(4, 'r2', 'README.md'), ...read(6, 'r3', 'batch'),
      ...shell(8, 's1', "mkdir -p batch && printf 'a' > batch/a.txt"),
      ...shell(10, 's2', "printf 'b' > batch/b.txt"),
      ...shell(12, 's3', "printf 'c' > batch/c.txt"),
      ev(14, 'run.completed', { runtimeThreadId: 't', process: {} }, 14)
    ]
    const thread = buildThread(events, { running: false })
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    expect(joined(activityTrace(details, events, traceOutcome(events, false)))).toBe('14s · ran 3 commands: mkdir and printf · 3 other tool calls')
  })

  it('does not count thinking as a tool call, on the line a person reads', () => {
    // Grok, pass 5 on 0.154.0: `activitySummary` had stopped counting
    // reasoning in 0.153.0 and this line had not. Two reads plus a thought
    // said `3 tool calls`; a think-only run said `1 tool call`. The
    // thought must be its own row and nothing else.
    const thinking = [
      ev(1, 'run.started', { runtimeThreadId: 't' }, 0),
      ev(2, 'step.started', { stepKind: 'reasoning', itemId: 'r1' }, 1),
      ev(3, 'step.completed', { stepKind: 'reasoning', itemId: 'r1', message: 'Need two reads, then answer.' }, 8),
      ev(4, 'tool.started', { itemId: 'a', toolKind: 'read', name: 'read', command: 'README.md', phase: 'started' }, 9),
      ev(5, 'tool.completed', { itemId: 'a', toolKind: 'read', name: 'read', command: 'README.md', phase: 'completed' }, 10),
      ev(6, 'tool.started', { itemId: 'b', toolKind: 'read', name: 'read', command: 'package.json', phase: 'started' }, 11),
      ev(7, 'tool.completed', { itemId: 'b', toolKind: 'read', name: 'read', command: 'package.json', phase: 'completed' }, 12),
      ev(8, 'run.completed', { runtimeThreadId: 't', process: {} }, 13)
    ]
    const thread = buildThread(thinking, { running: false })
    const activity = thread.find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    expect(details.some((detail) => detail.kind === 'reasoning')).toBe(true)
    expect(joined(activityTrace(details, thinking, traceOutcome(thinking, false)))).toBe('13s · thought 7s · 2 tool calls')

    const only = [
      ev(1, 'run.started', { runtimeThreadId: 't' }, 0),
      ev(2, 'step.started', { stepKind: 'reasoning', itemId: 'r1' }, 1),
      ev(3, 'step.completed', { stepKind: 'reasoning', itemId: 'r1', message: 'Just thinking.' }, 8),
      ev(4, 'run.completed', { runtimeThreadId: 't', process: {} }, 9)
    ]
    const onlyThread = buildThread(only, { running: false })
    const onlyActivity = onlyThread.find((item) => item.type === 'activity')
    const onlyDetails = onlyActivity?.type === 'activity' ? onlyActivity.details : []
    const line = joined(activityTrace(onlyDetails, only, traceOutcome(only, false)))
    expect(line).not.toMatch(/tool call/)
    expect(onlyActivity?.type === 'activity' && onlyActivity.summary).toBe('No tool activity')
  })

  it('names the one command it ran, rather than counting it', () => {
    /*
     * "1 tool call" counts a thing you cannot see without opening the fold --
     * the same complaint as the `1 notice` chip. A finished turn should read
     * as a bubble, one line and two replies, and that only works if the line
     * says what happened (design, 2026-09-08).
     *
     * Through `shellCommandText`, so a Windows run does not spend the whole
     * summary on the powershell preamble before reaching the command. That is
     * what `WRAPPED` is: the command exactly as the host builds it.
     */
    const WRAPPED = '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -NoProfile -Command "seq 1 300"'
    const events = [
      ev(1, 'run.started', { runtimeThreadId: 't', process: {} }, 0),
      ev(2, 'tool.started', { itemId: 'a', toolKind: 'command_execution', name: 'shell', command: WRAPPED, phase: 'started' }, 1),
      ev(3, 'tool.completed', { itemId: 'a', toolKind: 'command_execution', name: 'shell', command: WRAPPED, phase: 'completed', output: '1\n2\n3', exitCode: 0 }, 12),
      ev(4, 'run.completed', { runtimeThreadId: 't', process: {} }, 12)
    ]
    const activity = buildThread(events, { running: false }).find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    const segments = activityTrace(details, events, traceOutcome(events, false))
    // Under `commands` now, not `calls`: a command is named, never counted
    // among the anonymous calls. The words a person reads are unchanged.
    const ran = segments.find((seg) => seg.key === 'commands')
    expect(ran?.text).toBe('ran seq 1 300')
    expect(ran?.text).not.toContain('powershell')
    expect(segments.find((seg) => seg.key === 'calls')).toBeUndefined()
  })

  it('says the one command is RUNNING while it has no result -- not that it ran (0.378)', () => {
    // An approval card up beside the fold said "Nothing has happened yet"
    // while the line said "ran echo LOCUST-ACP-OK > approved.txt"
    // (drive-copilot-approve-each, 0.377; every runtime's Approve each read
    // the same). No result and a live turn: running, or waiting on the card.
    const started = [
      ev(1, 'run.started', { runtimeThreadId: 't', process: {} }, 0),
      ev(2, 'tool.started', { itemId: 'a', toolKind: 'command_execution', name: 'shell', command: 'echo hi', phase: 'started' }, 1)
    ]
    const live = buildThread(started, { running: true }).find((item) => item.type === 'activity')
    const liveDetails = live?.type === 'activity' ? live.details : []
    expect(activityTrace(liveDetails, started, traceOutcome(started, true)).find((seg) => seg.key === 'commands')?.text).toBe('running echo hi')
    const done = [
      ...started,
      ev(3, 'tool.completed', { itemId: 'a', toolKind: 'command_execution', name: 'shell', command: 'echo hi', phase: 'completed', exitCode: 0 }, 2),
      ev(4, 'run.completed', { runtimeThreadId: 't', process: {} }, 2)
    ]
    const after = buildThread(done, { running: false }).find((item) => item.type === 'activity')
    const afterDetails = after?.type === 'activity' ? after.details : []
    expect(activityTrace(afterDetails, done, traceOutcome(done, false)).find((seg) => seg.key === 'commands')?.text).toBe('ran echo hi')
  })

  it('counts several commands and says how they came out', () => {
    // Naming one of several would be arbitrary, so several get a count -- and
    // it carries what came back, which is the whole of Astra's first ask: a
    // turn said what it CHANGED and never what it RAN, so a run that edited
    // three files and a run that edited three files and proved them read
    // identically (2026-09-11).
    const events = [
      ev(1, 'run.started', { runtimeThreadId: 't', process: {} }, 0),
      ev(2, 'tool.started', { itemId: 'a', toolKind: 'command_execution', name: 'shell', command: 'ls', phase: 'started' }, 1),
      ev(3, 'tool.completed', { itemId: 'a', toolKind: 'command_execution', name: 'shell', command: 'ls', phase: 'completed', exitCode: 0 }, 2),
      ev(4, 'tool.started', { itemId: 'b', toolKind: 'command_execution', name: 'shell', command: 'pwd', phase: 'started' }, 3),
      ev(5, 'tool.completed', { itemId: 'b', toolKind: 'command_execution', name: 'shell', command: 'pwd', phase: 'completed', exitCode: 0 }, 4),
      ev(6, 'run.completed', { runtimeThreadId: 't', process: {} }, 4)
    ]
    const activity = buildThread(events, { running: false }).find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    const segments = activityTrace(details, events, traceOutcome(events, false))
    // NAMED, not graded. `all exit 0` was the one segment on this line where
    // the app aggregated into a verdict (design agent, 2026-09-11).
    expect(segments.find((seg) => seg.key === 'commands')?.text).toBe('ran ls and pwd')
    expect(segments.find((seg) => seg.key === 'calls')).toBeUndefined()
  })

  it('carries the notices it used to only count, as sentences on the fold', () => {
    // The positive control for the change above: dropping the chip must not
    // mean dropping the fact. This diagnostic arrives BEFORE any tool call and
    // is not a run-level error, so the thread's own gate drops it -- which is
    // exactly the set that used to be counted and never shown.
    const events = [
      ev(1, 'run.started', { runtimeThreadId: 't', process: {} }, 0),
      ev(2, 'adapter.diagnostic', { code: 'codex.setup', level: 'info', terminal: false, message: 'Skill descriptions were shortened.' }, 1),
      ev(3, 'tool.started', { itemId: 'a', toolKind: 'shell', name: 'Bash', command: 'ls', phase: 'started' }, 2),
      ev(4, 'tool.completed', { itemId: 'a', toolKind: 'shell', name: 'Bash', command: 'ls', phase: 'completed', output: 'README.md' }, 3),
      ev(5, 'run.completed', { runtimeThreadId: 't', process: {} }, 4)
    ]
    const activity = buildThread(events, { running: false }).find((item) => item.type === 'activity')
    const notices = activity?.type === 'activity' ? activity.notices : undefined
    expect(notices).toEqual([{ level: 'info', message: 'Skill descriptions were shortened.', source: 'claude' }])
    // And it is NOT also drawn as a thread diagnostic, which would say it twice.
    expect(buildThread(events, { running: false }).some((item) => item.type === 'diagnostic')).toBe(false)
  })

  it('says a subagent did not report, in amber, only once the turn is over', () => {
    const events = [
      ev(1, 'run.started', { runtimeThreadId: 't' }, 0),
      ev(2, 'tool.started', { itemId: 'a', toolKind: 'tool_use', name: 'Agent', command: 'Search', phase: 'started' }, 1),
      ev(3, 'run.cancelled', { process: {} }, 30)
    ]
    const live = buildThread(events.slice(0, 2), { running: true })
    const liveDetails = live.find((item) => item.type === 'activity')
    expect(joined(activityTrace(liveDetails?.type === 'activity' ? liveDetails.details : [], events.slice(0, 2), 'running'))).toBe('1s · asked 1 subagent')
    const over = buildThread(events, { running: false })
    const details = over.find((item) => item.type === 'activity')
    const segments = activityTrace(details?.type === 'activity' ? details.details : [], events, traceOutcome(events, false))
    expect(joined(segments)).toBe('stopped at 30s · nothing was changed · asked 1 subagent · it did not report')
    expect(segments.find((seg) => seg.key === 'subagents')?.tone).toBe('amber')
  })

  it('has the duration as its floor, and words a stop honestly', () => {
    expect(joined(activityTrace([], [ev(1, 'run.started', { runtimeThreadId: 't' }, 0), ev(2, 'run.completed', { runtimeThreadId: 't', process: {} }, 275)], 'completed'))).toBe('4m 35s')
    expect(durationText(3_960_000)).toBe('1h 06m')
    expect(traceOutcome([ev(1, 'run.failed', { message: 'x' }, 0)], false)).toBe('failed')
  })
})

describe('utilisation, in the words of the spec', () => {
  it('reads the fullest window and words the sentence', () => {
    const said = '5-hour window 67% used · resets 2026-09-06T02:10:00.000Z · 7-day window 53% used · resets 2026-09-07T07:00:00.000Z'
    const before = new Date('2026-09-06T01:00:00.000Z')
    expect(usagePercent(said, before)).toBe(67)
    expect(usageWindowSentence(said, before)).toMatch(/^67% of the 5-hour window used, resets .+ · 53% of the 7-day window, resets .+$/)
    expect(usagePercent('nothing')).toBeUndefined()
  })

  /*
   * 0.406, Colin: "my claude code usage hasnt seem to have updated". His
   * reading was from Locust's last Claude run the night before, and its
   * 5-hour window had reset hours ago -- still drawn as 20% used.
   */
  it('does not count a window whose reset has passed, and says it has reset', () => {
    const said = '7-day window 66% used · resets 2026-09-28T07:00:00.000Z · 5-hour window 20% used · resets 2026-09-27T05:30:00.000Z · from a run at 2026-09-27T01:23:19.233Z'
    const later = new Date('2026-09-27T15:30:00.000Z')
    expect(usagePercent(said, later)).toBe(66)
    const sentence = usageWindowSentence(said, later)
    expect(sentence).toMatch(/^66% of the 7-day window used, resets .+ · the 5-hour window has reset since \(as of Locust's last run .+\)$/)
    expect(sentence).not.toContain('20%')
  })

  it('says where a reading came from: the last run in Locust, or the account', () => {
    expect(usageReadOf('5-hour window 20% used · from a run at 2026-09-27T01:23:19.233Z')).toEqual({ kind: 'run', at: '2026-09-27T01:23:19.233Z' })
    expect(usageReadOf('primary 10% used · as of 2026-09-27T15:00:00.000Z')).toEqual({ kind: 'account', at: '2026-09-27T15:00:00.000Z' })
    expect(usageReadOf('5-hour window 20% used')).toBeUndefined()
    expect(usageReadLine('5-hour window 20% used · from a run at 2026-09-27T01:23:19.233Z')).toMatch(/^From Locust's last run on it, .+\. Use outside Locust since then isn't counted\.$/)
    expect(usageReadLine('primary 10% used · as of 2026-09-27T15:00:00.000Z')).toMatch(/^Read from your account at .+\.$/)
  })
})

describe('the model an alias turned out to mean (0.35.2)', () => {
  // Colin, 2026-09-06: "in model list they are just listed as sonnet, fable,
  // and opus". The picker had always been willing to show the real name; it
  // was reading the START record, which for Claude Code repeats the alias it
  // was given, so it never learned one however many runs had happened.
  const mission = (fields: Record<string, unknown>) =>
    ({ peerMessages: [], createdAt: '2026-09-06T05:00:00.000Z', ...fields }) as never

  const completed = (resolvedModel?: string) => ({
    type: 'run.completed',
    occurredAt: '2026-09-06T05:00:09.000Z',
    payload: { ...(resolvedModel === undefined ? {} : { resolvedModel }) }
  })

  it('learns it from the result, which is the only record that states it', () => {
    const names = resolvedModelNames([
      mission({
        missionId: 'm1',
        runtime: 'claude',
        model: 'sonnet',
        events: [completed('claude-sonnet-5')]
      })
    ])
    expect(names.get('claude:sonnet')).toBe('claude-sonnet-5')
  })

  it('learns nothing from a run that never named one', () => {
    const names = resolvedModelNames([
      mission({ missionId: 'm1', runtime: 'claude', model: 'sonnet', events: [completed()] })
    ])
    expect(names.get('claude:sonnet')).toBeUndefined()
  })
})

describe('Cursor reports files from inside its own copy of the project', () => {
  // Measured on a real Cursor Agent run, 2026-09-07: a two-line edit produced
  // eleven file rows, each one wearing a path under ~/.cursor/projects/, with
  // the filename pushed off the end of a one-line row. The directory name is
  // the workspace path with its separators flattened to hyphens, so the match
  // is certain rather than guessed.
  const WS = String.raw`C:\Users\<home>\code\streaks`

  it('reads the mirrored path as the file you would open yourself', () => {
    expect(
      relativePath(
        String.raw`C:\Users\<home>\.cursor\projects\C-Users-<home>-code-streaks\src\streak.js`,
        WS
      )
    ).toBe('src/streak.js')
  })

  it('matches case-insensitively, as Windows paths do', () => {
    expect(
      relativePath(
        String.raw`c:\users\<home>\.cursor\projects\c-users-<home>-code-streaks\src\cli.js`,
        WS
      )
    ).toBe('src/cli.js')
  })

  it('leaves another project\u2019s mirror whole, because there the location is the point', () => {
    const other = String.raw`C:\Users\<home>\.cursor\projects\C-Users-<home>-code-other\src\streak.js`
    expect(relativePath(other, WS)).toBe(other)
  })

  it('is not fooled by a folder that merely looks like the mirror', () => {
    const decoy = String.raw`C:\Users\<home>\notes\.cursor\projects\unrelated\a.js`
    expect(relativePath(decoy, WS)).toBe(decoy)
  })
})

describe('a run that could edit and edited nothing says so', () => {
  // Cursor Agent said "Applying the two edits to notes.ts now", reported
  // completed, and left the file untouched (measured 2026-09-07). The trace
  // read `41s · thought 3s · asked 4 subagents · 14 tool calls` and never
  // mentioned files, because the count is only drawn when it is above zero --
  // so nothing on screen contradicted the model's account of itself.
  const readTool = (): NormalizedRuntimeEvent[] => [
    event('tool.started', { itemId: 't1', toolKind: 'read', name: 'Read' }),
    event('tool.completed', { itemId: 't1', toolKind: 'read', name: 'Read' })
  ]

  const traceOf = (mayEdit: boolean | undefined): string =>
    activityTrace(
      [{ kind: 'tool', name: 'Read', settled: true }],
      readTool(),
      'completed',
      undefined,
      mayEdit
    )
      .map((segment) => segment.text)
      .join(' · ')

  it('states it when the run was allowed to change files', () => {
    expect(traceOf(true)).toContain('no files changed')
  })

  it('says nothing of the sort on a read-only run, where it is the point', () => {
    expect(traceOf(false)).not.toContain('no files changed')
  })

  it('says nothing when the caller does not know what the run was allowed', () => {
    expect(traceOf(undefined)).not.toContain('no files changed')
  })

  it('is plain, not amber: asking a question changes nothing either', () => {
    const files = activityTrace(
      [{ kind: 'tool', name: 'Read', settled: true }],
      readTool(),
      'completed',
      undefined,
      true
    ).find((segment) => segment.key === 'files')
    expect(files?.tone).toBeUndefined()
  })
})

describe('an earlier turn says it changed nothing too', () => {
  // `mayEdit` was passed for the newest turn and nowhere else, so scrolling up
  // in a conversation showed exactly the silence 0.38.9 exists to break: a run
  // that was allowed to edit, finished, and touched nothing looked the same as
  // one that had never been asked to.
  const readOnce = (): NormalizedRuntimeEvent[] => [
    event('tool.started', { itemId: 'r1', toolKind: 'read', name: 'Read' }),
    event('tool.completed', { itemId: 'r1', toolKind: 'read', name: 'Read' })
  ]
  const traceOf = (options: { readonly running: boolean; readonly mayEdit?: boolean }): string => {
    const items = buildThread(readOnce(), options)
    const activity = items.find((item) => item.type === 'activity')
    return activity?.type === 'activity' ? activity.trace.map((segment) => segment.text).join(' · ') : ''
  }

  it('says it on a finished turn that was allowed to edit', () => {
    expect(traceOf({ running: false, mayEdit: true })).toContain('no files changed')
  })

  it('still says nothing when that turn was read-only', () => {
    expect(traceOf({ running: false, mayEdit: false })).not.toContain('no files changed')
  })
})

describe('the same file spelled two ways is one file', () => {
  // A first outside tester, on OpenCode: "Alpha UI: 3s · 1 tool call · 2 files
  // · +2 −0. Git: one line in one file." A single append counted twice.
  //
  // pathKey folds separators and case but not absolute-vs-relative, and a
  // runtime that reports one edit as `README.md` and another as
  // `/home/you/proj/README.md` is describing the same file both times.
  const patchFor = (path: string) => ({
    text: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1 +1,2 @@\n a\n+ALPHA-TOUCHED\n`,
    added: 1,
    removed: 0,
    truncated: false
  })
  const WORKSPACE = '/home/you/locust-sample'
  const filesOf = (details: readonly ActivityDetail[]): string | undefined =>
    activityTrace(details, [], 'completed', undefined, undefined, WORKSPACE).find(
      (segment) => segment.key === 'files'
    )?.text

  it('counts a relative and an absolute spelling as one', () => {
    expect(
      filesOf([
        { kind: 'edit', name: 'README.md', settled: true, patch: patchFor('README.md') },
        {
          kind: 'edit',
          name: '/home/you/locust-sample/README.md',
          settled: true,
          patch: patchFor('/home/you/locust-sample/README.md')
        }
      ])
    ).toBe('1 file')
  })

  it('counts one edit reported twice as one', () => {
    // MEASURED, OpenCode, one append to one file: the fold drew
    // `notes.md MODIFIED +1 -0` twice, the summary said `1 file +2 -0`, and
    // git said `1 0 notes.md`.
    const twice = [
      { kind: 'edit' as const, name: 'notes.md', settled: true, patch: patchFor('notes.md') },
      { kind: 'edit' as const, name: 'notes.md', settled: true, patch: patchFor('notes.md') }
    ]
    expect(filesOf(twice)).toBe('1 file')
    expect(activityCounts(twice)).toEqual({ added: 1, removed: 0 })
    expect(activityEntries(twice).filter((entry) => entry.kind === 'file')).toHaveLength(1)
  })

  it('counts one edit reported relatively and absolutely as one', () => {
    // The measured shape: OpenCode restated the same append with the other
    // path spelling, the fold drew `notes.md MODIFIED +1 -0` twice, and the
    // line said `1 file +2 -0` against git's `1 0 notes.md`.
    const bothWays = [
      { kind: 'edit' as const, name: 'notes.md', settled: true, patch: patchFor('notes.md') },
      {
        kind: 'edit' as const,
        name: `${WORKSPACE}/notes.md`,
        settled: true,
        patch: patchFor(`${WORKSPACE}/notes.md`)
      }
    ]
    expect(filesOf(bothWays)).toBe('1 file')
    expect(activityCounts(bothWays, WORKSPACE)).toEqual({ added: 1, removed: 0 })
    expect(activityEntries(bothWays, WORKSPACE).filter((entry) => entry.kind === 'file')).toHaveLength(1)
  })

  it('still counts two real edits to one file as two', () => {
    // The second edit is diffed against a file the first already changed, so
    // its rows differ -- both are real work and both are drawn.
    const two = [
      { kind: 'edit' as const, name: 'notes.md', settled: true, patch: patchFor('notes.md') },
      {
        kind: 'edit' as const,
        name: 'notes.md',
        settled: true,
        patch: {
          text: 'diff --git a/notes.md b/notes.md\n--- a/notes.md\n+++ b/notes.md\n@@ -9 +9,2 @@\n c\n+BETA-TOUCHED\n',
          added: 1,
          removed: 0,
          truncated: false
        }
      }
    ]
    expect(activityCounts(two)).toEqual({ added: 2, removed: 0 })
    expect(activityEntries(two).filter((entry) => entry.kind === 'file')).toHaveLength(2)
  })

  it('still counts two genuinely different files as two', () => {
    expect(
      filesOf([
        { kind: 'edit', name: 'README.md', settled: true, patch: patchFor('README.md') },
        { kind: 'edit', name: 'notes.md', settled: true, patch: patchFor('notes.md') }
      ])
    ).toBe('2 files')
  })
})

describe('"no files changed" is a claim, not a default', () => {
  // MEASURED 2026-09-07, one teammate on OpenCode and one on Cursor writing in
  // one folder at once: Wren's card read `25s · 4 tool calls · no files
  // changed · 1 notice` while wren-note.txt sat on disk, correct, written by
  // that run. Its runtime reported no edit of its own, and the overlap
  // suppressed the host's git inference -- so the app knew nothing, which is
  // not the same as knowing nothing happened.
  const overlapNotice = {
    type: 'adapter.diagnostic',
    occurredAt: '2026-09-07T12:00:00.000Z',
    payload: {
      level: 'info',
      code: 'host.shared_workspace',
      message: 'Another teammate was working in this folder at the same time.'
    }
  } as unknown as NormalizedRuntimeEvent
  const filesText = (events: readonly NormalizedRuntimeEvent[]): string | undefined =>
    activityTrace([], events, 'completed', undefined, true).find((segment) => segment.key === 'files')?.text

  it('is withheld when the host has said it cannot tell what changed', () => {
    expect(filesText([overlapNotice])).toBeUndefined()
  })

  it('is still said for a lone run that genuinely changed nothing', () => {
    // The control. This line exists to break the silence that invites someone
    // to assume a run worked, so suppressing it everywhere would be the worse
    // bug -- it must only go where a contradicting notice stands beside it.
    expect(filesText([])).toBe('no files changed')
  })
})

/**
 * The newest finished turn keeps its work on screen.
 *
 * While a run is going the live step narrates it -- "Thinking", then each tool
 * as it is called -- and the moment it ended every bit of that was replaced by
 * one collapsed line. So the work vanished at exactly the moment a person
 * turns back to look at it (Colin, 2026-09-08: "the thoughts and tool calls
 * disappear after an agent is done ... we want that to stay so they can see
 * after the fact or if they missed it").
 */
describe('where a turn’s steps are drawn (0.491)', () => {
  /*
   * The fold that opened itself when a turn ended is gone: a turn's steps are
   * lines among the things said, the same while it runs and after, so there
   * is nothing to open and nothing that closes behind the person. The foot --
   * files and totals -- is drawn once the turn has ended.
   */
  const withTool = [
    event('tool.started', { itemId: 't1', toolKind: 'read', name: 'read', command: 'notes.md', phase: 'started' }),
    event('tool.completed', { itemId: 't1', toolKind: 'read', name: 'read', command: 'notes.md', phase: 'completed' })
  ]
  const kinds = (options: { running: boolean; latestTurn?: boolean }) =>
    buildThread(withTool, options).flatMap((entry) => (entry.type === 'steps' ? [`steps:${String(entry.finished)}`] : entry.type === 'activity' ? [`foot:${String(entry.finished)}`] : []))

  it('draws the step as a group, finished or not, earlier turn or latest', () => {
    expect(kinds({ running: false, latestTurn: true })).toEqual(['steps:true', 'foot:true'])
    expect(kinds({ running: false, latestTurn: false })).toEqual(['steps:true', 'foot:true'])
    expect(kinds({ running: true, latestTurn: true })).toEqual(['steps:false', 'foot:false'])
  })
})

describe('a turn sent with files attached', () => {
  // Colin asked for attachments; the host names them for the runtime in a line
  // above the message. That line was reaching the person as well: their own
  // bubble opened with an instruction they had not written, in the style that
  // says they wrote it.
  const sent = withAttachments('What is the passphrase?', ['NOTES.md', 'src/keys.ts'])

  it('shows what was typed, not what was sent', () => {
    expect(turnPromptLine({ prompt: sent })).toBe('What is the passphrase?')
    expect(turnPromptLine({ prompt: sent })).not.toMatch(/Read these/)
    expect(turnPromptLine({ prompt: sent })).not.toMatch(/NOTES\.md/)
  })

  it('still reports the files, so nothing is hidden by the fix', () => {
    expect(turnAttachments({ prompt: sent })).toEqual(['NOTES.md', 'src/keys.ts'])
  })

  it('leaves an ordinary turn exactly as it was', () => {
    expect(turnPromptLine({ prompt: 'Fix the parser.' })).toBe('Fix the parser.')
    expect(turnAttachments({ prompt: 'Fix the parser.' })).toEqual([])
  })

  it('claims no attachments on a turn the person did not type', () => {
    // A relayed or handed-off turn's prompt is machine-written; nothing
    // attached files to it, and reading paths out of one would invent them.
    expect(turnAttachments({ prompt: sent, startedBy: { kind: 'relay', hop: 1 } })).toEqual([])
    // A routine step and a room post ARE the person's own words.
    expect(turnAttachments({ prompt: sent, startedBy: { kind: 'routine', routineId: 'rt_1', step: 0 } })).toEqual([
      'NOTES.md',
      'src/keys.ts'
    ])
  })
})

describe('what a command printed', () => {
  it('keeps a short output whole, and asks for no elision control', () => {
    // Under the budget: no tail, nothing omitted, and so nothing drawn to say
    // that nothing was left out. `git status --short` with three lines should
    // not carry a line count and a button explaining an absence.
    expect(boundedShellOutput('one\ntwo\nthree')).toEqual({
      head: 'one\ntwo\nthree',
      tail: '',
      omitted: 0,
      total: 3,
      text: 'one\ntwo\nthree'
    })
  })

  it('keeps BOTH ends of a long one, as two halves the caller can put a control between', () => {
    // THE test, and the same rule message truncation follows. The interesting
    // line is as often the last as the first -- an error, an exit summary, the
    // answer. `seq 1 1200` is the cheerful case; `npm install` ending in a
    // permission error is the one that matters.
    const many = Array.from({ length: 1_200 }, (_, index) => `line ${String(index + 1)}`).join('\n')
    const { head, tail, omitted, total, text } = boundedShellOutput(many)
    expect(head.split('\n')).toHaveLength(SHELL_OUTPUT_HEAD_LINES)
    expect(tail.split('\n')).toHaveLength(SHELL_OUTPUT_TAIL_LINES)
    expect(head).toContain('line 1')
    expect(tail).toContain('line 1200')
    expect(omitted).toBe(1_200 - MAX_SHELL_OUTPUT_LINES)
    expect(total).toBe(1_200)
    // The joined form still exists for anything that wants one block.
    expect(text.split('\n').length).toBeLessThan(MAX_SHELL_OUTPUT_LINES + 3)
  })

  it('bounds by LINES rather than pixels, so nothing needs an inner scroller', () => {
    /*
     * Eight and eight. It was 200 lines in a 320px box with `overflow: auto`,
     * which put a scrolling region inside a scrolling thread and drew a 300px
     * black rectangle showing "1" through "19" above the two lines the
     * teammate actually said (design, 2026-09-08).
     *
     * Sixteen is also the boundary the drawing's third case sits on: a
     * 16-line output shows whole, with no button and no count.
     */
    expect(MAX_SHELL_OUTPUT_LINES).toBe(16)
    const sixteen = Array.from({ length: 16 }, (_, index) => String(index + 1)).join('\n')
    expect(boundedShellOutput(sixteen).omitted).toBe(0)
    const seventeen = `${sixteen}\n17`
    expect(boundedShellOutput(seventeen).omitted).toBe(1)
  })

  it('does not end on the blank line almost every command leaves behind', () => {
    // Driven on a real `seq 1 300`, whose output ends with a newline: the tail
    // read as an empty line instead of `300`, which undercuts the one promise
    // this bounding makes.
    const many = `${Array.from({ length: 400 }, (_, index) => String(index + 1)).join('\n')}\n`
    const { text } = boundedShellOutput(many)
    expect(text.split('\n').at(-1)).toBe('400')
  })

  it('says how much it left out rather than trailing off', () => {
    const many = Array.from({ length: 500 }, () => 'x').join('\n')
    expect(boundedShellOutput(many).text).toContain('more lines')
  })
})

describe('a shell row and its output', () => {
  const shellDetail = (output?: string): ActivityDetail => ({
    kind: 'shell',
    name: 'seq 1 1200',
    settled: true,
    failed: false,
    ...(output === undefined ? {} : { output })
  }) as unknown as ActivityDetail

  it('carries the output the runtime reported', () => {
    const entry = activityEntries([shellDetail('1\n2\n3')], undefined)[0]
    expect(entry?.kind).toBe('shell')
    expect(entry?.kind === 'shell' ? entry.output : undefined).toBe('1\n2\n3')
  })

  it('carries nothing where the runtime reported nothing', () => {
    // Undefined is the ordinary case: only the Codex exec stream reports
    // command output. The row reads its own data rather than promising an
    // expansion it cannot deliver on four other runtimes.
    const entry = activityEntries([shellDetail()], undefined)[0]
    expect(entry?.kind === 'shell' ? entry.output : 'missing').toBeUndefined()
  })

  it('keeps an empty output distinct from an absent one', () => {
    /*
     * This used to assert that an empty output was dropped, the same as an
     * absent one. Both then arrived at the row as `undefined`, and the row
     * could not tell "the runtime reports no output at all" -- true of five of
     * the six -- from "it ran the command and it printed nothing".
     *
     * The design's fourth case is the second of those: one line saying `no
     * output`, rather than an empty black rectangle. The case was written in
     * ActivityCard and was UNREACHABLE, because this normaliser had already
     * collapsed the two. Caught by rendering the card
     * (`command-output.test.tsx`), not by any test of this function.
     *
     * The intent the old test protected -- an empty output must not offer an
     * expansion it cannot deliver -- is unchanged and is asserted below: the
     * row stays static. What changed is that it now says so.
     */
    const empty = activityEntries([shellDetail('')], undefined)[0]
    expect(empty?.kind === 'shell' ? empty.output : 'missing').toBe('')
    const absent = activityEntries([shellDetail()], undefined)[0]
    expect(absent?.kind === 'shell' ? absent.output : 'missing').toBeUndefined()
  })
})

/*
 * Astra, 2026-09-11: a turn's line says what it CHANGED and never what it
 * RAN. The ledger has had the commands and their exit codes the whole time.
 *
 * The rule these all share: NO INFERENCE ABOUT WHAT A COMMAND MEANS. Nothing
 * here decides that `pnpm test` is a test and `ls` is not. It says what ran
 * and what came back; whether that is evidence is the reader's call, and an
 * app guessing at proof is worse than one staying quiet.
 */
describe('what a turn ran, and what came back', () => {
  const detail = (over: Partial<ActivityDetail> = {}): ActivityDetail => ({
    kind: 'shell',
    name: 'pnpm test',
    tool: 'shell',
    settled: true,
    ...over
  }) as ActivityDetail

  it('says nothing at all when nothing was run', () => {
    expect(commandsRunText(commandsRun([], true))).toBeUndefined()
    expect(commandsRun([], true)).toEqual({ ran: 0, nonZero: 0, unsettled: 0 })
  })

  /*
   * The success case NAMES the commands; it does not grade them.
   *
   * It shipped as `ran 4 commands - all exit 0`, and the design agent found
   * the defect: that is the one segment where the app aggregates into a
   * verdict, and "all" plus "0" is as close to PASSED as you get without
   * typing it. Their test for grading -- "could it be wrong?" -- catches it:
   * literally no, but what it COMMUNICATES can be, and that gap is the bug.
   */
  it('names the commands rather than grading them', () => {
    expect(commandsRunText(commandsRun([detail(), detail()], true), ['pnpm check', 'tsc --noEmit'])).toEqual({
      text: 'ran pnpm and tsc',
      amber: false
    })
  })

  it('names two and counts the rest, because four command lines do not fit', () => {
    // The count is the COMMANDS and "more" is the other KINDS: this read
    // "ran pnpm, tsc and 2 more" with the commands and the kinds mixed in
    // one number (Yurt's beta report, #6).
    const four = [detail(), detail(), detail(), detail()]
    expect(commandsRunText(commandsRun(four, true), ['pnpm check', 'tsc', 'node x.mjs', 'git status'])?.text)
      .toBe('ran 4 commands: pnpm, tsc and 2 more')
  })

  it('names each kind once, so twelve commands of two kinds read as twelve', () => {
    // Yurt's fat turn: one `mkdir && printf`, eleven `printf`, under the line
    // "ran mkdir, printf and 10 more".
    const twelve = Array.from({ length: 12 }, () => detail())
    const names = ["mkdir -p batch && printf 'a' > batch/a.txt", ...'bcdefghijkl'.split('').map((c) => `printf '${c}' > batch/${c}.txt`)]
    expect(commandsRunText(commandsRun(twelve, true), names)?.text).toBe('ran 12 commands: mkdir and printf')
  })

  it("reads past Claude Code's cd in front of a command", () => {
    // Colin's frame of a long run, 2026-09-23: "ran cd, cd and 8 more".
    const three = [detail(), detail(), detail()]
    const names = ['cd C:\\work\\app && npm test', 'cd "C:\\work\\my app" && git status', 'cd /work/app; npm run build']
    expect(commandsRunText(commandsRun(three, true), names)?.text).toBe('ran 3 commands: npm and git')
  })

  it('falls back to a count when no command name was recorded', () => {
    // A count is not a verdict either -- it is only less useful.
    expect(commandsRunText(commandsRun([detail(), detail()], true), [])?.text).toBe('ran 2 commands')
    expect(commandsRunText(commandsRun([detail()], true), [])?.text).toBe('ran 1 command')
  })

  it('says nothing about how they came out when they all came out fine', () => {
    // The whole ruling in one assertion: no "all exit 0", no "passed", no
    // "checked", nothing the reader could take as the app's judgement.
    const said = commandsRunText(commandsRun([detail(), detail()], true), ['pnpm check', 'tsc'])?.text ?? ''
    for (const verdict of ['exit 0', 'pass', 'ok', 'success', 'checked', 'verified']) {
      expect(said.toLowerCase(), verdict).not.toContain(verdict)
    }
  })

  it('counts a non-zero exit, and says so in amber', () => {
    const run = commandsRun([detail({ exitCode: 0 }), detail({ exitCode: 1 })], true)
    expect(run).toMatchObject({ ran: 2, nonZero: 1 })
    expect(commandsRunText(run)).toEqual({ text: 'ran 2 commands · 1 exited non-zero', amber: true })
  })

  it('counts a command that failed outright as non-zero, however it failed', () => {
    expect(commandsRun([detail({ failed: true })], true).nonZero).toBe(1)
  })

  it('does not call a command that never reported a pass', () => {
    // A turn that is over and a command that never reported is not a pass,
    // and not a failure either.
    const run = commandsRun([detail({ exitCode: 0 }), detail({ settled: false })], true)
    expect(run).toMatchObject({ ran: 2, nonZero: 0, unsettled: 1 })
    expect(commandsRunText(run)).toEqual({ text: 'ran 2 commands · 1 did not report', amber: true })
  })

  it('does not count an unfinished turn\u2019s open command as unreported', () => {
    // It has not finished YET. Only a turn that is over can say a command
    // never came back.
    expect(commandsRun([detail({ settled: false })], false).unsettled).toBe(0)
  })

  it('a non-zero exit outranks an unreported one: the failure is the news', () => {
    const run = commandsRun([detail({ exitCode: 2 }), detail({ settled: false })], true)
    expect(commandsRunText(run)?.text).toBe('ran 2 commands · 1 exited non-zero')
  })
})

describe('everything a teammate said in one turn', () => {
  /*
   * The room drew a turn from `the last message marked final`, falling back
   * to `the latest message` while none was -- so progress appeared as it was
   * written and VANISHED the moment the turn finished, the final message
   * replacing everything before it. Colin, 2026-09-13: "the agents initial
   * messages are properly appearing in rooms chat but then when final message
   * is sent out it disappears."
   *
   * Measured against his own thread, which had always drawn all four: the
   * room showed one.
   */
  const said = (texts: readonly { text: string; final: boolean }[]): NormalizedRuntimeEvent[] =>
    texts.map((entry, index) => ({
      id: `e${String(index)}`,
      runId: 'run_1',
      missionId: 'mission_1',
      sequence: index,
      type: 'message.delta',
      occurredAt: '2026-09-13T00:00:00.000Z',
      sourceAdapter: 'cursor',
      payload: { itemId: `msg_${String(index)}`, operation: 'replace', text: entry.text, final: entry.final, evidence: { redacted: true } }
    }) as unknown as NormalizedRuntimeEvent)

  it('keeps what was said before the last message, not just the last', () => {
    const text = turnText(said([
      { text: 'I will check the workspace notes first.', final: true },
      { text: 'I have Street prints; next I will pin live spots.', final: true },
      { text: 'NVDA stays in the 215-250 band.', final: true }
    ]))
    expect(text).toContain('workspace notes')
    expect(text).toContain('Street prints')
    expect(text).toContain('215-250')
  })

  it('keeps them in the order they were said', () => {
    const text = turnText(said([{ text: 'first', final: true }, { text: 'second', final: true }]))
    expect(text.indexOf('first')).toBeLessThan(text.indexOf('second'))
  })

  it('does not change while a turn is still going, then finishing', () => {
    // The exact shape of the disappearance: mid-turn there is no final
    // message, and the earlier text must survive one arriving.
    const during = turnText(said([{ text: 'working on it', final: false }]))
    const after = turnText(said([{ text: 'working on it', final: false }, { text: 'here is the answer', final: true }]))
    expect(during).toContain('working on it')
    expect(after).toContain('working on it')
    expect(after).toContain('here is the answer')
  })

  it('is empty when nothing was said, rather than a blank paragraph', () => {
    expect(turnText([])).toBe('')
    expect(turnText(said([{ text: '   ', final: true }]))).toBe('')
  })
})

describe('a plan does not claim nothing changed when something did', () => {
  /*
   * Lifting the plan out of the fold so it is always visible had a cost I
   * did not see: the plan item also carries "Plan mode — nothing was
   * changed", and it only ever existed on a turn with NO activity, so that
   * sentence was true by construction. Shown on every turn, it started
   * appearing over runs that had just made thirty tool calls.
   *
   * Colin, 2026-09-14, with a plan, the sentence, and `9 tool calls` in one
   * frame: "this is definitely a bug".
   *
   * The fact is now carried rather than assumed.
   */
  const planned = { plan: [{ step: 'Read it', status: 'pending' }] }

  it('says so when the turn really did only plan', () => {
    const thread = buildThread([event('plan.updated', planned), event('run.completed', {})], { running: false })
    const shown = thread.find((item) => item.type === 'plan')
    expect(shown?.type === 'plan' ? shown.touchedNothing : undefined).toBe(true)
  })

  it('does not say so when the turn ran tools', () => {
    const thread = buildThread(
      [
        event('plan.updated', planned),
        event('tool.started', { itemId: 'i1', name: 'edit', command: 'src/a.ts' }),
        event('tool.completed', { itemId: 'i1', name: 'edit', command: 'src/a.ts', status: 'ok' }),
        event('run.completed', {})
      ],
      { running: false }
    )
    const shown = thread.find((item) => item.type === 'plan')
    expect(shown, 'the plan is still shown').toBeDefined()
    expect(shown?.type === 'plan' ? shown.touchedNothing : 'missing').toBeUndefined()
  })
})

describe('the group still growing at the end of a running turn (0.584)', () => {
  // Colin, 2026-10-04, of a silent ten-minute Flash turn: "its stacking on one
  // line". The trailing group is live: drawn open, its rows arriving.
  const at = '2026-10-04T05:00:00.000Z'
  it('is live', () => {
    const items = buildThread([toolStart('t1', 'Read', 'a.txt'), toolDone('t1'), toolStart('t2', 'Read', 'b.txt'), toolDone('t2')], { running: true, startedAt: at })
    const steps = items.filter((i) => i.type === 'steps')
    expect(steps).toHaveLength(1)
    expect(steps[0]).toMatchObject({ live: true })
  })
  it('is not live once the model is talking after it, nor when the turn is over (controls)', () => {
    const talking = buildThread([toolStart('t1', 'Read', 'a.txt'), toolDone('t1'), delta('m1', 'Found it.', 'append')], { running: true, startedAt: at })
    expect(talking.some((i) => i.type === 'steps')).toBe(true)
    expect(talking.find((i) => i.type === 'steps')).not.toMatchObject({ live: true })
    const over = buildThread([toolStart('t1', 'Read', 'a.txt'), toolDone('t1')], { running: false, startedAt: at })
    expect(over.find((i) => i.type === 'steps')).not.toMatchObject({ live: true })
  })
})

describe('a group the run has moved past (0.594)', () => {
  // Colin, 2026-10-04, of a Grok run on Cursor: every group that had ever been
  // live stayed open and the thread read as a wall. Claude Code keeps one step
  // open, the one under way; the rest fold to their line.
  const at = '2026-10-04T05:00:00.000Z'
  it('is superseded once a later group is live, and the live one is not', () => {
    const items = buildThread(
      [toolStart('t1', 'Read', 'a.txt'), toolDone('t1'), delta('m1', 'Found it.', 'append', true), toolStart('t2', 'Read', 'b.txt'), toolDone('t2')],
      { running: true, startedAt: at }
    )
    const steps = items.filter((i) => i.type === 'steps')
    expect(steps).toHaveLength(2)
    expect(steps[0]).toMatchObject({ superseded: true })
    expect(steps[0]).not.toMatchObject({ live: true })
    expect(steps[1]).toMatchObject({ live: true })
    expect(steps[1]).not.toMatchObject({ superseded: true })
  })
  it('is not superseded by the model talking after it (the reply is not a step), nor once the turn is over (controls)', () => {
    // The last group keeps the 9/8 rule: the work stays in view when the run ends, reply or no reply.
    const talking = buildThread([toolStart('t1', 'Read', 'a.txt'), toolDone('t1'), delta('m1', 'Found it.', 'append')], { running: true, startedAt: at })
    expect(talking.find((i) => i.type === 'steps')).not.toMatchObject({ superseded: true })
    const over = buildThread(
      [toolStart('t1', 'Read', 'a.txt'), toolDone('t1'), delta('m1', 'Found it.', 'append', true), toolStart('t2', 'Read', 'b.txt'), toolDone('t2')],
      { running: false, startedAt: at }
    )
    expect(over.filter((i) => i.type === 'steps').some((i) => (i as { superseded?: boolean }).superseded === true)).toBe(false)
  })
})

describe('a lone thought on its step line (0.594)', () => {
  const thought = { kind: 'reasoning', name: 'thought', settled: true, durationMs: 3_000, output: 'I will check the Roth IRA and related news. The account is read-only.' } as unknown as ActivityDetail
  it('previews its words closed, and not open, where the words are underneath', () => {
    const closed = stepsLine([thought], false).segments[0]?.text ?? ''
    expect(closed.startsWith('Thought for 3s: I will check the Roth IRA and related news.')).toBe(true)
    expect(closed.length).toBeLessThan(100)
    expect(stepsLine([thought], false, undefined, { open: true }).segments[0]?.text).toBe('Thought for 3s')
  })
})

describe("Cursor's scratch files (0.594)", () => {
  it('are named for what they are, not by their UUID', () => {
    const WS = 'C:/Users/x/projects/streaks'
    expect(relativePath('C:/Users/x/.cursor/projects/C-Users-x-claude/agent-tools/3a5a3562-ffd2-47fc-b4bc-8d2d64fcd8a2.txt', WS)).toBe("Cursor's saved tool result")
    // A project file of the same shape elsewhere keeps its name (control).
    expect(relativePath('C:/Users/x/projects/streaks/agent-tools/notes.txt', WS)).toBe('agent-tools/notes.txt')
  })
})

describe('a turn with no work still says what the runtime said (0.586)', () => {
  // The cross-runtime output pass, 2026-10-04: three recorded runs drew
  // "(nothing in the thread)" -- a Copilot policy warning that came before any
  // work, and two Antigravity failures whose reason reached only the rail.
  const policy = "Third-party MCP servers are disabled by your organization's Copilot policy."
  it('draws a notice that came before any work when there is no fold to put it under', () => {
    const items = buildThread([
      event('adapter.diagnostic', { level: 'warning', code: 'copilot.session_warning', message: policy, terminal: false }),
      event('run.completed', { process: {} })
    ], { running: false })
    expect(items.find((i) => i.type === 'diagnostic')).toMatchObject({ level: 'warning', message: policy })
  })
  it('keeps it under the fold when there was work (control)', () => {
    const items = buildThread([
      event('adapter.diagnostic', { level: 'warning', code: 'copilot.session_warning', message: policy, terminal: false }),
      toolStart('t1', 'Read', 'a.txt'),
      toolDone('t1'),
      event('run.completed', { process: {} })
    ], { running: false })
    expect(items.some((i) => i.type === 'diagnostic' && i.message === policy)).toBe(false)
    const foot = items.find((i) => i.type === 'activity')
    expect(foot?.type === 'activity' && (foot.notices ?? []).some((n) => n.message === policy)).toBe(true)
  })
  it('names why a run failed before it said anything', () => {
    const why = 'Antigravity could not run it: model no-such-model is not recognized'
    const items = buildThread([event('run.failed', { kind: 'process-failed', message: why, runtimeTerminal: 'failed', process: {} })], { running: false })
    expect(items.find((i) => i.type === 'diagnostic')).toMatchObject({ level: 'error', message: why })
  })
  it('does not say a failure twice when a diagnostic already carries it (control)', () => {
    const said = 'The provider turned the request away.'
    const items = buildThread([
      event('adapter.diagnostic', { level: 'error', code: 'codex.runtime_error', message: said, terminal: true }),
      event('run.failed', { kind: 'process-failed', message: said, runtimeTerminal: 'failed', process: {} })
    ], { running: false })
    expect(items.filter((i) => i.type === 'diagnostic' && i.message === said)).toHaveLength(1)
  })
})

describe('the text a file row shows for a path (0.596)', () => {
  const workspace = 'C:\\Users\\dev\\.claude'
  it('is relative inside the folder, as before', () => {
    expect(displayPath('C:\\Users\\dev\\.claude\\report.md', workspace)).toBe('report.md')
    expect(displayPath('pebble/hello.txt', workspace)).toBe('pebble/hello.txt')
  })
  it('is the last two segments of a path elsewhere, behind an ellipsis', () => {
    expect(displayPath('C:\\Users\\dev\\Documents\\Codex\\checkout\\apps\\desktop\\src\\components\\Thread.tsx', workspace)).toBe('…/components/Thread.tsx')
    expect(displayPath('/home/dev/project/src/main.rs', workspace)).toBe('…/src/main.rs')
  })
  it('keeps a short absolute path whole, and names a scratch file for what it is', () => {
    expect(displayPath('C:\\notes\\todo.txt', workspace)).toBe('C:\\notes\\todo.txt')
    expect(displayPath('C:\\Users\\dev\\.cursor\\projects\\x\\agent-tools\\3a5a3562.txt', workspace)).toBe("Cursor's saved tool result")
  })
})

describe('a named file the host saw change but could not read (0.597)', () => {
  it('reads as changed on the runtime\'s own row, with its word, not as "did not report"', () => {
    // Colin's ledger, 10/04: his workspace is ~/.claude, whose files the 0.489 secrets rule never
    // reads, so every report a teammate wrote there said "did not report the change" -- while the
    // host had seen the file change by name.
    const thread = buildThread([
      event('tool.started', { itemId: 'w', toolKind: 'write_to_file', name: 'write_to_file', command: 'report.md', phase: 'started' }),
      event('tool.completed', { itemId: 'w', toolKind: 'write_to_file', name: 'write_to_file', command: 'report.md', phase: 'completed' }),
      event('run.completed', { process: {} }),
      event('tool.started', { itemId: 'obs', toolKind: 'observed_edit', name: 'edit', command: 'report.md', status: 'reported by the runtime, changed on disk', phase: 'started' }),
      event('tool.completed', { itemId: 'obs', toolKind: 'observed_edit', name: 'edit', command: 'report.md', status: 'reported by the runtime, changed on disk', phase: 'completed' })
    ], { running: false })
    const activity = thread.find((item) => item.type === 'activity')
    const entries = activity?.type === 'activity' ? activityEntries(activity.details) : []
    const rows = entries.filter((entry) => (entry.kind === 'unreported' || entry.kind === 'tool') && /report\.md/.test(entry.name))
    expect(rows).toHaveLength(1)
    expect(rows[0]).toMatchObject({ kind: 'unreported', observed: true, tool: 'write_to_file' })
  })
})

describe('the live line while nothing has arrived (0.602)', () => {
  it("says the start's phase when given one, else Starting", () => {
    const withPhase = buildThread([], { running: true, startedAt: '2026-10-04T20:00:00.000Z', startingLabel: 'Reading the folder' }).find((item) => item.type === 'live-step')
    expect(withPhase?.type === 'live-step' ? withPhase.label : undefined).toBe('Reading the folder')
    const plain = buildThread([], { running: true, startedAt: '2026-10-04T20:00:00.000Z' }).find((item) => item.type === 'live-step')
    expect(plain?.type === 'live-step' ? plain.label : undefined).toBe('Starting')
  })
})

/*
 * A CODEX RUN'S LINE AND ROWS READ AS WORDS (0.610). Colin's screenshots of
 * 2026-10-04: the row read `Get-Content 'C:\\Users\\<home>\\.codex\\...'`, the
 * live line "Using imageGeneration", and the plan step "Inspect the
 * screenshot and relevant visual-design notes. · step 1 of 3".
 */
describe('a Codex run, said as words', () => {
  const ESCAPED = '"C:\\\\Windows\\\\System32\\\\WindowsPowerShell\\\\v1.0\\\\powershell.exe" -Command "Get-Content \'C:\\\\Users\\\\me\\\\.codex\\\\skills\\\\imagegen\\\\SKILL.md\'; Get-Content notes.md"'

  it('shows the command as the shell got it, when Codex escaped every backslash', () => {
    expect(shellCommandText(ESCAPED)).toBe("Get-Content 'C:\\Users\\me\\.codex\\skills\\imagegen\\SKILL.md'; Get-Content notes.md")
  })

  it('leaves a command alone when one backslash is not half of a pair', () => {
    const plain = '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "Get-Content C:\\Users\\me\\notes.md"'
    expect(shellCommandText(plain)).toBe('Get-Content C:\\Users\\me\\notes.md')
  })

  it('still undoes an escaped quote inside an escaped payload', () => {
    const quoted = '"C:\\\\Windows\\\\powershell.exe" -Command "rg \\"streak\\" C:\\\\work"'
    expect(shellCommandText(quoted)).toBe('rg "streak" C:\\work')
  })

  it('says Codex\'s image tool in words, on the live line and in the steps line', () => {
    expect(liveActionLine({ kind: 'tool', name: 'imageGeneration', tool: 'imageGeneration', settled: false } as ActivityDetail)).toBe('Generating an image')
    expect(stepsLine([{ kind: 'tool', name: 'imageGeneration', tool: 'imageGeneration', settled: true } as ActivityDetail], true).segments[0]?.text).toBe('Made an image')
    expect(stepsLine([
      { kind: 'tool', name: 'imageGeneration', tool: 'imageGeneration', settled: true },
      { kind: 'tool', name: 'imageGeneration', tool: 'imageGeneration', settled: true }
    ] as ActivityDetail[], true).segments[0]?.text).toBe('Made 2 images')
    // A tool it has no words for is still named, as before.
    expect(liveActionLine({ kind: 'tool', name: 'fooBar', tool: 'fooBar', settled: false } as ActivityDetail)).toBe('Using fooBar')
  })
})

describe('a tool the app has words for, as a row (0.610)', () => {
  it('reads as what it did, not the program name', () => {
    const entries = activityEntries([{ kind: 'tool', name: 'imageGeneration', tool: 'imageGeneration', settled: true } as ActivityDetail])
    expect(entries.map((entry) => entry.kind === 'tool' ? entry.name : '')).toEqual(['Made an image'])
  })
})

describe('Locust’s git tool and OpenCode’s failed tool call, in words (0.818)', () => {
  const call = (tool: string, settled = true): ActivityDetail => ({ kind: 'tool', name: tool, tool, settled }) as ActivityDetail
  it('says the git a run with no shell ran, as a shell’s git reads', () => {
    expect(liveActionLine(call('locust_git_diff', false))).toBe('Running git diff')
    expect(activityEntries([call('locust_git_branches')]).map((entry) => entry.kind === 'tool' ? entry.name : '')).toEqual(['Ran git branch'])
    expect(railToolName({ name: 'locust_git_log' })).toBe('Ran git log')
    const said = (tools: readonly string[]): string | undefined => stepsLine(tools.map((tool) => call(tool)), true).segments[0]?.text
    expect(said(['locust_git_diff'])).toBe('Ran git diff')
    expect(said(['locust_git_diff', 'locust_git_diff'])).toBe('Ran git diff 2 times')
    expect(said(['locust_git_status', 'locust_git_diff', 'locust_git_log'])).toBe('Ran git status, diff and log')
    expect(said(['locust_git_status', 'locust_git_diff', 'locust_git_log', 'locust_git_show'])).toBe('Ran git 4 times')
    // Another server's tool that only looks like it keeps its own name.
    expect(said(['locust_git_push'])).toBe('Used locust_git_push')
  })

  it('says a tool call OpenCode could not run, never "used invalid"', () => {
    expect(stepsLine([call('invalid')], true).segments[0]?.text).toBe('Made a tool call that did not work')
    expect(activityEntries([call('invalid')]).map((entry) => entry.kind === 'tool' ? entry.name : '')).toEqual(['Made a tool call that did not work'])
  })
})
