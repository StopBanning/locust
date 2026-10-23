import { appendFile, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'

import { createFileMissionLedger } from '@teammate/mission-store'
import type { MissionLedger, MissionLedgerMetadata, RecoveredMission } from '@teammate/mission-store'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { MISSION_RECORD_SAVE_CHANNEL } from '../shared/ipc.js'
import { approvalNotRecordedNote } from './approval-record-note.js'
import { SAVED_RULE_DENIAL, missionRecordMarkdown, rawRecordJson, recordFileName, recordTurns } from './mission-export.js'

/**
 * SAVE THE RECORD (0.575): one conversation as one Markdown file.
 *
 * The ledger is seeded through its OWN writer and read back through its own
 * reader, so the file is checked against what a ledger really hands over --
 * not against a hand-built object that could drift from it.
 *
 * What is seeded is what the ledger can hold: a call the person declined, a
 * call a saved rule's sentence names, a call the mode refused, a hand-off to
 * another runtime with its checkpoint, a diff the ledger kept short, a result
 * it was too large to keep, a message between teammates, a run that failed, a
 * check the person ran, and a file whose tail was torn. It cannot hold an
 * approval that was allowed, or who answered a card; the file says so, and
 * these tests pin that it does rather than pretend.
 */

const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

const ISO = (minute: number, second = 0): string => `2026-10-02T09:${String(minute).padStart(2, '0')}:${String(second).padStart(2, '0')}.000Z`
const SAVED_AT = '2026-10-03T08:00:00.000Z'

const metadata = (over: Partial<MissionLedgerMetadata> & Pick<MissionLedgerMetadata, 'missionId' | 'prompt' | 'runtime' | 'createdAt'>): MissionLedgerMetadata =>
  ({
    runId: `run_${over.missionId}`,
    model: over.runtime === 'codex' ? 'gpt-test' : 'sonnet',
    requestedRouteId: over.runtime,
    resolvedRouteId: `${over.runtime}:default`,
    cliVersion: over.runtime === 'codex' ? '0.160.0' : '2.1.0',
    workspaceId: 'ws_test',
    sandbox: 'workspace-write',
    mode: 'approve-each',
    executionPolicyVersion: 1,
    ...over
  }) as MissionLedgerMetadata

const processEvidence = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 1, inputDeliveryFailed: false,
  outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false,
  startedAt: ISO(0), finishedAt: ISO(1)
}

class Events {
  private n = 0
  constructor(private readonly mission: MissionLedgerMetadata) {}
  private make(type: string, payload: Record<string, unknown>, minute: number, second: number): NormalizedRuntimeEvent {
    this.n += 1
    return {
      id: `${this.mission.runId}:${String(this.n)}`, runId: this.mission.runId, missionId: this.mission.missionId, sequence: this.n,
      occurredAt: ISO(minute, second), sourceAdapter: this.mission.runtime, type, payload
    } as unknown as NormalizedRuntimeEvent
  }
  started(minute: number): NormalizedRuntimeEvent { return this.make('run.started', { runtimeThreadId: 'thread_1', evidence: { redacted: true } }, minute, 0) }
  said(text: string, minute: number, itemId = 'msg_1'): NormalizedRuntimeEvent {
    return this.make('message.delta', { itemId, operation: 'append', text, final: true, evidence: { redacted: true } }, minute, 5)
  }
  tool(type: 'tool.started' | 'tool.completed' | 'tool.failed', itemId: string, fields: Record<string, unknown>, minute: number, second: number): NormalizedRuntimeEvent {
    return this.make(type, {
      itemId, toolKind: 'command_execution', name: 'shell', phase: type === 'tool.started' ? 'started' : 'completed',
      evidence: { redacted: true }, ...fields
    }, minute, second)
  }
  completed(minute: number, resolvedModel?: string): NormalizedRuntimeEvent {
    return this.make('run.completed', { runtimeThreadId: 'thread_1', ...(resolvedModel === undefined ? {} : { resolvedModel }), process: processEvidence }, minute, 50)
  }
  failed(message: string, minute: number): NormalizedRuntimeEvent {
    return this.make('run.failed', { kind: 'process-failed', message, runtimeThreadId: 'thread_1', runtimeTerminal: 'failed', process: processEvidence }, minute, 50)
  }
  diagnostic(code: string, message: string, minute: number): NormalizedRuntimeEvent {
    return this.make('adapter.diagnostic', { level: 'info', code, message, terminal: false, evidence: { redacted: true } }, minute, 55)
  }
}

const DIFF = [
  'diff --git a/README.md b/README.md', '--- a/README.md', '+++ b/README.md', '@@ -1,2 +1,2 @@', ' # Locust', '-Teh best tool', '+The best tool'
].join('\n')

interface Seeded {
  readonly ledger: MissionLedger
  readonly root: string
  readonly turns: readonly RecoveredMission[]
}

/**
 * Three turns of one conversation. Turn 1 on Codex: a command that ran, a
 * decline, a saved rule's denial, a diff, a diff too large to keep whole, a
 * result too large to keep. Turn 2 a reply handed to Claude, which a mode
 * refused one call of, and which ended in failure. Turn 3 a follow-up whose
 * file is then torn at the tail.
 */
async function seed(options: { readonly tearTurnThree?: boolean } = {}): Promise<Seeded> {
  const root = await mkdtemp(join(tmpdir(), 'locust-record-'))
  roots.push(root)
  const ledger = createFileMissionLedger({ rootDirectory: root, now: () => new Date(ISO(30)) })

  const one = metadata({ missionId: 'mission_one', prompt: 'Tidy the build folder and fix the typo in the README.', runtime: 'codex', createdAt: ISO(0) })
  await ledger.createMission(one)
  const a = new Events(one)
  await ledger.appendEvents(one.missionId, [
    a.started(0),
    a.said('I will run the tests first, then clean up.', 0),
    a.tool('tool.started', 'c1', { command: 'npm test' }, 1, 0),
    a.tool('tool.completed', 'c1', { command: 'npm test', output: 'Tests 12 passed', exitCode: 0, status: 'completed', durationMs: 4200 }, 1, 5),
    a.tool('tool.started', 'c2', { command: 'rm -rf build' }, 1, 10),
    a.tool('tool.failed', 'c2', { command: 'rm -rf build', status: 'declined', output: 'The person declined this, and said: use the build script instead' }, 1, 12),
    a.tool('tool.started', 'c3', { command: 'git push origin main' }, 1, 20),
    a.tool('tool.failed', 'c3', {
      command: 'git push origin main', status: 'declined',
      output: `Denied in Locust. The person declined this, and said: ${SAVED_RULE_DENIAL} never run git push for Wren in this folder.`
    }, 1, 22),
    a.tool('tool.started', 'c4', { command: 'sed -i s/Teh/The/ README.md' }, 2, 0),
    a.tool('tool.completed', 'c4', { toolKind: 'file_change', name: 'Edit', command: 'README.md', status: 'completed', patch: { text: DIFF, added: 1, removed: 1, truncated: false } }, 2, 3),
    a.tool('tool.completed', 'c5', {
      toolKind: 'file_change', name: 'Write', command: 'data/big.txt', status: 'completed',
      patch: { text: '--- a/data/big.txt\n+++ b/data/big.txt\n@@ -1,3 +1,3 @@\n-aaa\n+bbb\n ccc', added: 61000, removed: 60000, truncated: true }
    }, 2, 10),
    a.tool('tool.completed', 'c6', { toolKind: 'tool_use', name: 'Write', command: 'media/clip.bin', status: 'result too large to keep', output: 'The result was too large to keep (900 KB). The teammate read it; Locust keeps results up to 256 KB.' }, 2, 20),
    a.said('Ran the tests, fixed the typo, and left build/ alone as asked.', 3, 'msg_2'),
    a.completed(3, 'gpt-test-2026-10')
  ])
  await ledger.createCheckpoint(one.missionId, 'route-switch')

  const two = metadata({
    missionId: 'mission_two', runtime: 'claude', createdAt: ISO(10), mode: 'plan',
    prompt: `Brief the host wrote: the earlier turns, summarised.\n\nThe person now asks:\n\nCan you review that change from a fresh pair of eyes?`,
    continuesFrom: { missionId: 'mission_one', checkpointEpoch: 1, reason: 'route-switch', leftOut: ['earlier'], leftOutByYou: ['summary'] }
  })
  await ledger.createMission(two)
  const b = new Events(two)
  await ledger.appendEvents(two.missionId, [
    b.started(10),
    b.tool('tool.failed', 'r1', { toolKind: 'tool_use', name: 'Bash', command: 'npm install left-pad', status: 'refused', output: 'Plan mode does not run commands.' }, 11, 0),
    b.said('I could not install anything in Plan mode, but the change reads well.', 11),
    b.failed('The connection dropped while the answer was being written.', 12)
  ])
  await ledger.appendPeerLinks(two.missionId, [{ direction: 'posted', messageId: 'msg_peer_1', peerTeammateId: 'tm_booty', occurredAt: ISO(11, 30) }])
  await ledger.appendEditCheck(two.missionId, { command: 'npm test', outcome: 'passed', newLines: ['Tests 12 passed'], unchanged: false, first: true, occurredAt: ISO(13) })

  const three = metadata({
    missionId: 'mission_three', prompt: 'Thanks. Now say it in one sentence.', runtime: 'claude', createdAt: ISO(20), mode: 'ask',
    continuesFrom: { missionId: 'mission_two', checkpointEpoch: 1, reason: 'follow-up' }
  })
  await ledger.createMission(three)
  const c = new Events(three)
  await ledger.appendEvents(three.missionId, [c.started(20), c.said('The typo is fixed and the tests pass.', 20), c.completed(21)])
  await ledger.flush()
  if (options.tearTurnThree === true) await appendFile(join(root, 'mission_three.jsonl'), '{"recordType":"mission.event","event":', 'utf8')

  const turns = (await recordTurns((id) => ledger.getMission(id), 'mission_three')).missions
  return { ledger, root, turns }
}

const record = (turns: readonly RecoveredMission[], over: Partial<Parameters<typeof missionRecordMarkdown>[0]> = {}): string =>
  missionRecordMarkdown({ missions: turns, teammate: 'Wren', folder: 'C:\\work\\locust', locustVersion: '0.575.0', savedAt: SAVED_AT, ...over })

/** The same turns as a ledger written before cards were recorded (v19) reads them. */
const before20 = (turns: readonly RecoveredMission[]): RecoveredMission[] => turns.map((turn) => ({ ...turn, schemaVersion: 19 as const, approvals: [] }))

const GOLDEN = new URL('./a-saved-record-says-only-what-the-ledger-holds.golden.md', import.meta.url)

describe('a saved record, from a ledger seeded through its own writer', () => {
  it('reads exactly as the golden file: every turn, every declined call, every command, every diff, how each ended', async () => {
    const { turns } = await seed({ tearTurnThree: true })
    const markdown = record(before20(turns))
    // `LOCUST_UPDATE_GOLDEN=1` rewrites it; the diff is then read by a person, not trusted.
    if (process.env.LOCUST_UPDATE_GOLDEN === '1') await writeFile(GOLDEN, markdown, 'utf8')
    expect(markdown).toBe(await readFile(GOLDEN, 'utf8'))
  })

  it('is the same file when it is written twice from the same ledger', async () => {
    const { turns } = await seed()
    expect(record(turns)).toBe(record(turns))
  })
})

describe('a reply saved alone', () => {
  it('brings every turn before it, oldest first', async () => {
    const { ledger } = await seed()
    const fromTheLast = await recordTurns((id) => ledger.getMission(id), 'mission_three')
    expect(fromTheLast.missions.map((turn) => turn.metadata.missionId)).toEqual(['mission_one', 'mission_two', 'mission_three'])
    const markdown = record(fromTheLast.missions)
    expect(markdown.indexOf('Tidy the build folder')).toBeGreaterThan(-1)
    expect(markdown.indexOf('Tidy the build folder')).toBeLessThan(markdown.indexOf('Can you review that change'))
    expect(markdown.indexOf('Can you review that change')).toBeLessThan(markdown.indexOf('Now say it in one sentence'))
    expect(markdown).toContain('- **Turns:** 3')
  })

  it('from the middle turn, brings its parent and not the turn that came after', async () => {
    const { ledger } = await seed()
    const fromTheMiddle = await recordTurns((id) => ledger.getMission(id), 'mission_two')
    expect(fromTheMiddle.missions.map((turn) => turn.metadata.missionId)).toEqual(['mission_one', 'mission_two'])
    expect(record(fromTheMiddle.missions)).not.toContain('Now say it in one sentence')
  })

  it('names a parent the ledger no longer holds instead of skipping it', async () => {
    const { ledger } = await seed()
    await ledger.deleteMission('mission_one')
    const chain = await recordTurns((id) => ledger.getMission(id), 'mission_three')
    expect(chain.missions.map((turn) => turn.metadata.missionId)).toEqual(['mission_two', 'mission_three'])
    expect(chain.missingParent).toBe('mission_one')
    const markdown = record(chain.missions, { missingParent: chain.missingParent! })
    expect(markdown).toContain('`mission_one`, is not in the ledger')
    expect(markdown).toContain('record incomplete')
  })

  it('says there is nothing to record rather than writing an empty report, when no turn is found', async () => {
    const { ledger } = await seed()
    const none = await recordTurns((id) => ledger.getMission(id), 'mission_nobody')
    expect(none.missions).toEqual([])
    expect(record([])).toContain('holds no turn of this conversation')
  })

  it('stops at a cycle instead of spinning', async () => {
    const { turns } = await seed()
    const [one] = turns
    const loop = { ...one!, metadata: { ...one!.metadata, continuesFrom: { missionId: one!.metadata.missionId, checkpointEpoch: 1, reason: 'follow-up' as const } } }
    const chain = await recordTurns(async () => loop, 'mission_one')
    expect(chain.missions).toHaveLength(1)
  })
})

describe('what the file says about approvals', () => {
  it('in a turn written before cards were recorded, lists a declined call as declined and does not say whether a click or a rule answered', async () => {
    const { turns } = await seed()
    const markdown = record(before20(turns))
    expect(markdown).toContain('- **Declined.** Asked: `rm -rf build`.')
    expect(markdown).toContain('> Recorded with the call: The person declined this, and said: use the build script instead')
    expect(markdown).toContain('It does not record which, and the words recorded with the call do not say.')
  })

  it('lists a denial whose recorded words name a saved rule as that, and quotes the rule\'s sentence', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('- **Denied, and the words recorded with it name a saved rule.** Asked: `git push origin main`. The call did not run.')
    expect(markdown).toContain(`${SAVED_RULE_DENIAL} never run git push for Wren in this folder.`)
  })

  it('lists a call the mode refused as refused, with no answer from a person claimed', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('- **Refused before it ran.** Asked: `npm install left-pad`.')
    expect(markdown).toContain('no answer from a person is recorded')
  })

  it('in turns written before cards were recorded, says in every one that an allowed approval and who answered are not in the ledger', async () => {
    const { turns } = await seed()
    const markdown = record(before20(turns))
    const said = markdown.match(/_Not in the ledger:_ an approval that was allowed, what the card asked in full, who answered it and how\./g) ?? []
    expect(said).toHaveLength(3)
    // And a call that ran is never described as approved.
    expect(markdown).not.toMatch(/approved by|was approved|allowed by/i)
  })

  it('says a turn with no declined or refused call has none recorded, instead of leaving the section out', async () => {
    const { turns } = await seed()
    expect(record(before20(turns))).toContain('No declined or refused call is recorded in this turn.')
  })
})

/**
 * CARDS ANSWERED ARE IN THE RECORD (0.576, ledger v20). The record export
 * found the ledger held no approval at all; now each answered card is its own
 * record, written where every answer passes, and the file lists them.
 */
describe('a saved record of turns that record their cards', () => {
  const answered = async (): Promise<string> => {
    const { ledger } = await seed()
    await ledger.appendApproval('mission_one', {
      approvalId: 'ap_1', kind: 'command', asked: 'Run a command\nnpm test', answer: 'allowed', by: 'card',
      askedAt: ISO(0, 59), occurredAt: ISO(1, 0)
    })
    await ledger.appendApproval('mission_one', {
      approvalId: 'ap_2', kind: 'command', asked: 'Run a command\nrm -rf build', answer: 'denied', by: 'card',
      words: 'use the build script instead', askedAt: ISO(1, 10), occurredAt: ISO(1, 12)
    })
    await ledger.appendApproval('mission_one', {
      approvalId: 'ap_3', kind: 'command', asked: 'Run a command\ngit push origin main', answer: 'denied', by: 'saved-rule',
      words: 'never run git push for Wren in this folder.', askedAt: ISO(1, 20), occurredAt: ISO(1, 21)
    })
    // v21 (0.616): allowed by the person's own Always on an earlier card, with no card of its own.
    await ledger.appendApproval('mission_one', {
      approvalId: 'ap_4', kind: 'command', asked: 'Run a command\nnpm run lint', answer: 'allowed', by: 'earlier-always',
      words: 'The Always given earlier in this run allows every command it runs.', askedAt: ISO(1, 30), occurredAt: ISO(1, 30)
    })
    await ledger.flush()
    const turns = (await recordTurns((id) => ledger.getMission(id), 'mission_three')).missions
    return record(turns)
  }

  it('lists each card answered: what it asked, the answer, and who gave it', async () => {
    const markdown = await answered()
    expect(markdown).toContain(`- **Allowed** by the person, on the card · command · asked ${ISO(0, 59)} · answered ${ISO(1, 0)}`)
    expect(markdown).toContain('> The card asked: Run a command\n> npm test')
    expect(markdown).toContain('- **Denied** by the person, on the card · command')
    expect(markdown).toContain('> Said with the answer: use the build script instead')
    expect(markdown).toContain('- **Denied** by a rule the person saved, before the card reached them · command')
    expect(markdown).toContain('- **Allowed** by the person’s Always on an earlier card of this run, with no card of its own · command')
    expect(markdown).toContain('> Said with the answer: The Always given earlier in this run allows every command it runs.')
  })

  it('points a declined call at the answer above instead of saying it cannot tell', async () => {
    const markdown = await answered()
    expect(markdown).toContain('- **Declined.** Asked: `rm -rf build`. The call did not run; the answer that declined it is listed above.')
    expect(markdown).not.toContain('It does not record which')
  })

  it('says what it records and what it still does not, and never that an allowed approval is missing', async () => {
    const markdown = await answered()
    expect(markdown).toContain('_Recorded:_ every card answered on this turn, with who answered it.')
    expect(markdown).not.toContain('_Not in the ledger:_ an approval that was allowed')
  })

  it('says a turn that records cards had none answered, rather than leaving the section out', async () => {
    const markdown = await answered()
    expect(markdown).toContain('No card was answered, and no call was declined or refused, in this turn.')
  })

  it('counts an answer the host could not write down, and does not claim every answer is there (0.587)', async () => {
    const { ledger, turns } = await seed()
    const three = turns.find((turn) => turn.metadata.missionId === 'mission_three')!
    // The note the host builds must be one the ledger's reader accepts: a refused
    // note would be unwritable, and a written-but-unreadable one would hide the turn.
    const note = approvalNotRecordedNote({ mission: three, runId: three.metadata.runId, missionId: 'mission_three', kind: 'command', answer: 'allowed', why: 'disk full', now: () => new Date(ISO(21, 30)) })!
    await ledger.appendEvents('mission_three', [note])
    await ledger.flush()
    const markdown = record((await recordTurns((id) => ledger.getMission(id), 'mission_three')).missions)
    const turnThree = markdown.slice(markdown.indexOf('## Turn 3'))
    expect(turnThree).toContain('_Recorded:_ every card answered on this turn should be here; 1 could not be written down, and the notes under "How it ended" say which.')
    expect(turnThree).not.toContain('with who answered it.')
    // The other turns, with nothing unwritten, still make the whole claim (control).
    expect(markdown.slice(0, markdown.indexOf('## Turn 3'))).toContain('_Recorded:_ every card answered on this turn, with who answered it.')
    expect(markdown).toContain('Note Locust wrote into the record (`host.approval-not-recorded`, ' + ISO(21, 30) + '): The answer to a card (command, allowed) could not be written to this record: disk full')
  })
})

describe('what the file says about commands and changes', () => {
  it('gives a command as recorded, with its exit code, how long it took and its output', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('completed · exit code 0 · 4.2 s')
    expect(markdown).toContain('```sh\nnpm test\n```')
    expect(markdown).toContain('Output as recorded:\n\n```\nTests 12 passed\n```')
  })

  it('gives a diff as the ledger holds it, with its counts', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('Diff as recorded: +1 −1')
    expect(markdown).toContain('```diff\ndiff --git a/README.md b/README.md')
  })

  it('says a diff the ledger kept short is too large to keep whole, and what the runtime reported for all of it', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('_Too large to keep whole.')
    expect(markdown).toContain('the runtime reported +61000 −60000 for the whole change')
  })

  it('quotes the ledger\'s own words where a result was too large to keep and no diff exists', async () => {
    const { turns } = await seed()
    expect(record(turns)).toContain("No diff is recorded. The ledger's own words: The result was too large to keep (900 KB).")
  })

  it('never turns a missing diff into an empty one', async () => {
    const { turns } = await seed()
    const [one] = turns
    const bare = {
      ...one!,
      events: one!.events.map((event) => (event.type === 'tool.completed' && event.payload.itemId === 'c4' ? ({ ...event, payload: { ...event.payload, patch: undefined } } as NormalizedRuntimeEvent) : event))
    }
    expect(record([bare])).toContain('_No diff is recorded for this change._')
  })

  it('keeps a code fence in a command from closing the fence around it', async () => {
    const { turns } = await seed()
    const [one] = turns
    const tricky = {
      ...one!,
      events: one!.events.map((event) =>
        (event.type === 'tool.started' || event.type === 'tool.completed') && event.payload.itemId === 'c1'
          ? ({ ...event, payload: { ...event.payload, command: 'echo "```"' } } as NormalizedRuntimeEvent)
          : event
      )
    }
    const markdown = record([tricky])
    expect(markdown).toContain('````sh\necho "```"\n````')
  })
})

describe('what the file says about who answered, hand-offs and how each turn ended', () => {
  it('names the runtime and model of every turn, and gives a switch to another runtime a line of its own', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('- **Turn 1** · Codex CLI · model `gpt-test`, which the runtime named `gpt-test-2026-10` · mode approve-each · sandbox workspace-write · CLI 0.160.0')
    expect(markdown).toContain('- ↪ **Handed over** from Codex CLI to Claude Code.')
    expect(markdown).toContain('- **Turn 2** · Claude Code · model `sonnet` · mode plan')
    expect(markdown).toContain('- **Turn 3** · Claude Code · model `sonnet` · mode ask')
    // Two Claude turns in a row: no second switch line.
    expect(markdown.match(/↪/g)).toHaveLength(1)
  })

  it('says a mode was not recorded when it was not, rather than naming a default', async () => {
    const { turns } = await seed()
    const [one] = turns
    const { mode: _mode, ...older } = one!.metadata
    expect(record([{ ...one!, metadata: older as MissionLedgerMetadata }])).toContain('mode not recorded (this turn predates it)')
  })

  it('records the hand-off with its checkpoint, and what the brief left out, and holds back the brief itself', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('- **Handed over** from Codex CLI, model `gpt-test`, which the runtime named `gpt-test-2026-10`, to Claude Code, model `sonnet`, at')
    expect(markdown).toContain('Resumed from checkpoint 1 (route-switch)')
    expect(markdown).toContain('The hand-over brief left out, to fit: earlier.')
    expect(markdown).toContain('The person chose to leave out of the brief: summary.')
    expect(markdown).not.toContain('Brief the host wrote')
    expect(markdown).toContain('> Can you review that change from a fresh pair of eyes?')
  })

  it('records a message between teammates by what the ledger holds, and says the text lives elsewhere', async () => {
    const { turns } = await seed()
    expect(record(turns)).toContain('**Posted a message to another teammate** (`tm_booty`)')
    expect(record(turns)).toContain('The message is held in the workroom, not in this ledger (message `msg_peer_1`).')
  })

  it('says how each turn ended: finished, failed with the runtime\'s words, and the check the person ran', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('**Finished.** The runtime reported the run complete')
    expect(markdown).toContain('**Failed** at 2026-10-02T09:12:50.000Z (process-failed).')
    expect(markdown).toContain('> The connection dropped while the answer was being written.')
    expect(markdown).toContain("The person's check, `npm test`, after this turn: **passed**")
  })

  it('says no ending is recorded for a turn that stops without one, and does not call it finished', async () => {
    const { turns } = await seed()
    const [one] = turns
    const cut = { ...one!, phase: 'interrupted' as const, events: one!.events.filter((event) => event.type !== 'run.completed') }
    const markdown = record([cut])
    expect(markdown).toContain('**No ending is recorded.**')
    expect(markdown).not.toContain('**Finished.**')
  })

  it('does not present a prompt the host wrote as the person\'s words', async () => {
    const { turns } = await seed()
    const [one] = turns
    const relayed = { ...one!, metadata: { ...one!.metadata, startedBy: { kind: 'relay' as const, hop: 2 } } }
    const markdown = record([relayed])
    expect(markdown).toContain('Nobody typed this turn: Locust started it so Wren could answer another teammate (automatic turn 2).')
    expect(markdown).not.toContain('### The person said')
  })

  it('says a hand-off nobody typed for carried the work on its own, and quotes nothing as the person\'s', async () => {
    const { turns } = await seed()
    const [, two] = turns
    const rescue = { ...two!, metadata: { ...two!.metadata, prompt: 'Brief the host wrote with no instruction in it.' } }
    const markdown = record([turns[0]!, rescue])
    expect(markdown).toContain('Nobody typed this turn. Locust carried the work to another runtime on its own')
    expect(markdown).not.toContain('Brief the host wrote with no instruction')
  })

  it('marks an edited turn as an edit and says the earlier version is not in the file', async () => {
    const { turns } = await seed()
    const [, , three] = turns
    const edited = { ...three!, metadata: { ...three!.metadata, continuesFrom: { ...three!.metadata.continuesFrom!, edited: true as const } } }
    expect(record([turns[0]!, turns[1]!, edited])).toContain('This turn is an edit of an earlier message')
  })
})

describe('what the file says about itself', () => {
  it('says "record readable" for a clean ledger, and that this is not tamper evidence', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('- **The record\'s own check:** **record readable.**')
    expect(markdown).toContain('This is not tamper evidence')
    expect(markdown).not.toContain('record incomplete')
  })

  it('says "record incomplete" and lists each problem when a ledger file was torn', async () => {
    const { turns } = await seed({ tearTurnThree: true })
    const markdown = record(turns)
    expect(markdown).toContain('**record incomplete.** 1 problem was found reading this conversation\'s ledger')
    expect(markdown).toContain('`truncated-tail`')
    expect(markdown).toContain('(mission_three)')
    expect(markdown).not.toContain('**record readable.**')
  })

  it('uses the words the app already says for the same two states', async () => {
    const status = (await import(new URL('../renderer/src/status.ts', import.meta.url).href)) as { ledgerVerificationLabel: (count: number) => string }
    const { turns } = await seed({ tearTurnThree: true })
    expect(record(turns)).toContain(`**${status.ledgerVerificationLabel(1)}.**`)
    expect(record(turns.slice(0, 2).map((turn) => ({ ...turn, issues: [] })))).toContain(`**${status.ledgerVerificationLabel(0)}.**`)
  })

  it('names the teammate, the folder as it is now, when it started and ended, the Locust version, and what is not recorded', async () => {
    const { turns } = await seed()
    const markdown = record(turns)
    expect(markdown).toContain('# Record of a conversation with Wren')
    expect(markdown).toContain('- **Folder:** C:\\work\\locust (where the teammate works now; the ledger holds only the workspace id `ws_test`)')
    expect(markdown).toContain('- **Started:** 2026-10-02T09:00:00.000Z')
    expect(markdown).toContain('- **Locust version:** 0.575.0, the build that wrote this file. The ledger does not record which build ran each turn.')
    const unnamed = record(turns, { folder: undefined as unknown as string, teammate: undefined as unknown as string })
    expect(unnamed).toContain('- **Teammate:** not recorded: the roster no longer names them')
    expect(unnamed).toContain('- **Folder:** not recorded. The ledger holds only the workspace id `ws_test`.')
  })

  it('does not put the dialog\'s warning in the file: that is said once, where the person decides', async () => {
    const { turns } = await seed()
    expect(record(turns)).not.toContain('Read it before you send it')
  })

  it('quotes what was said so a heading in a reply cannot become part of the file\'s own outline', async () => {
    const { turns } = await seed()
    const [, , three] = turns
    const loud = { ...three!, events: three!.events.map((event) => (event.type === 'message.delta' ? ({ ...event, payload: { ...event.payload, text: '# A heading\n\n- a list' } } as NormalizedRuntimeEvent) : event)) }
    const markdown = record([loud])
    expect(markdown).toContain('> # A heading\n>\n> - a list')
    expect(markdown).not.toMatch(/^# A heading/m)
  })
})

describe('the raw record beside it', () => {
  it('holds the events exactly as the ledger holds them', async () => {
    const { turns } = await seed()
    const parsed = JSON.parse(rawRecordJson(turns, '0.575.0', SAVED_AT)) as { missions: { events: unknown[]; metadata: unknown }[]; writtenBy: string }
    expect(parsed.writtenBy).toBe('Locust 0.575.0')
    expect(parsed.missions.map((mission) => mission.events)).toEqual(turns.map((turn) => JSON.parse(JSON.stringify(turn.events))))
    expect(parsed.missions.map((mission) => mission.metadata)).toEqual(turns.map((turn) => JSON.parse(JSON.stringify(turn.metadata))))
  })
})

describe('the file name', () => {
  it('is Locust record - teammate - date.md', () => {
    expect(recordFileName('Wren', new Date(2026, 9, 3, 12))).toBe('Locust record - Wren - 2026-10-03.md')
  })
  it('carries nothing a file name cannot hold, and has a word when the teammate is unknown', () => {
    expect(recordFileName('Wr:en/<Code>?', new Date(2026, 0, 5))).toBe('Locust record - WrenCode - 2026-01-05.md')
    expect(recordFileName(undefined, new Date(2026, 0, 5))).toBe('Locust record - conversation - 2026-01-05.md')
    expect(recordFileName('***', new Date(2026, 0, 5))).toBe('Locust record - conversation - 2026-01-05.md')
  })
})

describe('how the channel is guarded and where the file goes', () => {
  const index = readFile(new URL('./index.ts', import.meta.url), 'utf8')
  const handler = async (): Promise<string> => {
    const lines = (await index).split('\n')
    const at = lines.findIndex((line) => line.includes('ipcMain.handle(MISSION_RECORD_SAVE_CHANNEL'))
    expect(at).toBeGreaterThan(-1)
    const end = lines.findIndex((line, offset) => offset > at && /^    ipcMain\.handle\(/.test(line))
    return lines.slice(at, end).join('\n')
  }

  it('answers only Locust\'s own window, on the first line', async () => {
    const body = (await handler()).split('\n')
    expect(body[1]).toContain('if (!fromOwnWindow(event))')
  })

  it('writes only where a native save dialog titled "Save the record" says, and uploads and opens nothing', async () => {
    const body = await handler()
    expect(body).toContain("title: 'Save the record'")
    expect(body).toContain('dialog.showSaveDialog(window')
    expect(body).toMatch(/Locust record|recordFileName\(/)
    for (const forbidden of ['shell.openPath', 'shell.openExternal', 'fetch(', 'net.request', 'https.request', 'http.request', 'showItemInFolder']) {
      expect(body, forbidden).not.toContain(forbidden)
    }
  })

  it('takes a turn and a tick from the window, and builds the text itself from the ledger', async () => {
    const body = await handler()
    expect(body).toContain('missionRecordMarkdown(')
    expect(body).toContain('missionLedger.getMission')
    expect(body).not.toMatch(/request\.(markdown|text|content|body)/)
  })

  it('is one channel, named once, and reaches the window through the bridge', async () => {
    expect(MISSION_RECORD_SAVE_CHANNEL).toBe('missions:save-record')
    const preload = await readFile(new URL('../shared/desktop-api.ts', import.meta.url), 'utf8')
    expect(preload).toContain('ipcRenderer.invoke(MISSION_RECORD_SAVE_CHANNEL, request)')
  })
})

describe('where the person finds it', () => {
  it('is an item in a conversation\'s right-click menu and in the command palette', async () => {
    const app = await readFile(new URL('../renderer/src/App.tsx', import.meta.url), 'utf8')
    const menu = app.slice(app.indexOf('const openMissionMenu'), app.indexOf('const checkUpdate'))
    expect(menu).toContain("label: 'Save the record…'")
    expect(menu).toContain('setSavingRecordOf(missionId)')
    const palette = app.slice(app.indexOf('<CommandPalette'))
    expect(palette).toContain("label: 'Save the record…'")
    expect(palette).toContain('setSavingRecordOf(terminalMissionId)')
  })

  it('has a dialog that says what the file holds once, and offers the raw record as a tick that starts unticked', async () => {
    const dialog = (await import(new URL('../renderer/src/components/SaveRecordDialog.tsx', import.meta.url).href)) as {
      SaveRecordDialog: (props: { missionId: string; onClose: () => void }) => ReturnType<typeof createElement>
      RECORD_CLAIM: string
    }
    const html = renderToStaticMarkup(createElement(dialog.SaveRecordDialog as never, { missionId: 'mission_three', onClose: () => undefined }))
    expect(dialog.RECORD_CLAIM).toBe('This file contains the conversation. Read it before you send it.')
    expect(html.split(dialog.RECORD_CLAIM)).toHaveLength(2)
    expect(html).toContain('Include the raw record (JSON, not scrubbed)')
    expect(html).toContain('Save the record…')
    expect(html).not.toMatch(/checked=""/)
  })
})
