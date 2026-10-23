import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { defangProtocolBlocks } from '../../shared/protocolTags.js'
import { INSTRUCTIONS_TAG, instructionsBlock, instructionsIn, interviewPrompt, isInterview } from '../../shared/interview.js'
import { MAX_TEAMMATE_INSTRUCTIONS, type PublicTeammate } from '../../shared/ipc.js'
import { instructionsRefusal } from './components/InstructionsCard.js'
import { Thread } from './components/Thread.js'

/**
 * A NEW TEAMMATE INTERVIEWS YOU (0.831, shared/interview.ts). The interview begins with the person's own ask, the
 * teammate writes its instructions in a block, and the card under that answer offers them in a box to change and
 * Save. Only an interview's answer draws the card; a block anywhere else is only words, and a quoted one is defanged.
 */
const WRITTEN = 'You are Ada. Answer in three bullet points at most.\nAsk before changing a file outside docs/.'
const answer = (text: string): NormalizedRuntimeEvent[] => [
  { id: 'm1', runId: 'run_1', missionId: 'mission_1', sequence: 1, occurredAt: '2026-10-23T12:00:01.000Z', sourceAdapter: 'opencode', type: 'message.delta',
    payload: { itemId: 'answer', operation: 'append', text, final: true, evidence: { redacted: true } } },
  { id: 'done', runId: 'run_1', missionId: 'mission_1', sequence: 2, occurredAt: '2026-10-23T12:00:02.000Z', sourceAdapter: 'opencode', type: 'run.completed',
    payload: { runtimeTerminal: 'completed', process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: '2026-10-23T12:00:00.000Z', finishedAt: '2026-10-23T12:00:02.000Z' }, evidence: { redacted: true } } }
] as unknown as NormalizedRuntimeEvent[]
const ADA = { teammateId: 'tm_ada', name: 'Ada', hue: 'teal', role: 'Research & Briefs', avatar: { shape: 'clover', face: 'smile', hue: 'teal' }, createdAt: '2026-10-23T01:00:00.000Z' } as unknown as PublicTeammate

const thread = (prompt: string, events: readonly NormalizedRuntimeEvent[], options: { readonly running?: boolean; readonly self?: PublicTeammate } = {}): string => renderToStaticMarkup(
  <Thread prompt={prompt} onOpenPeerRun={() => undefined} earlierTurns={[]} events={events}
    running={options.running === true} restoredMission={undefined} error={undefined} errorIsPersistence={false} startedAt={undefined}
    approvals={[]} onDecide={() => undefined} onAnswerQuestion={() => undefined} decidingIds={[]} cancelled={false}
    handoff={undefined} peers={{ self: options.self ?? ADA, teammates: [], messages: [], notices: [] }}
    onSaveInstructions={async () => undefined} />
)

describe('a new teammate interviews you', () => {
  it('begins with the person’s ask, and tells the teammate how to answer', () => {
    const prompt = interviewPrompt('Ada', 'Research & Briefs')
    expect(isInterview(prompt)).toBe(true)
    expect(prompt).toContain('You are Ada, Research & Briefs')
    expect(prompt).toContain('one short question at a time')
    expect(prompt).toContain('use no tool, read no file and make no plan')
    expect(prompt).toContain('or when I say I am done')
    expect(prompt).toContain(`<${INSTRUCTIONS_TAG}>`)
    expect(prompt).toContain('no password, key or other secret')
    expect(isInterview('Write the release notes')).toBe(false)
    expect(isInterview(undefined)).toBe(false)
  })

  it('reads the last block outside code, trimmed and bounded', () => {
    expect(instructionsBlock(`Here they are.\n<${INSTRUCTIONS_TAG}>\n  ${WRITTEN}  \n</${INSTRUCTIONS_TAG}>`)).toBe(WRITTEN)
    expect(instructionsBlock(`<${INSTRUCTIONS_TAG}>first</${INSTRUCTIONS_TAG}> then <${INSTRUCTIONS_TAG}>second</${INSTRUCTIONS_TAG}>`)).toBe('second')
    // One shown in code is an example, not the answer.
    expect(instructionsBlock('```\n<locust-instructions>an example</locust-instructions>\n```')).toBeUndefined()
    expect(instructionsBlock(`<${INSTRUCTIONS_TAG}>   </${INSTRUCTIONS_TAG}>`)).toBeUndefined()
    expect(instructionsBlock(`<${INSTRUCTIONS_TAG}>${'x'.repeat(MAX_TEAMMATE_INSTRUCTIONS + 50)}</${INSTRUCTIONS_TAG}>`)).toHaveLength(MAX_TEAMMATE_INSTRUCTIONS)
    expect(instructionsIn(answer(`<${INSTRUCTIONS_TAG}>${WRITTEN}</${INSTRUCTIONS_TAG}>`))).toBe(WRITTEN)
    // Quoted into another teammate's prompt, the block cannot act.
    expect(defangProtocolBlocks(`<${INSTRUCTIONS_TAG}>x</${INSTRUCTIONS_TAG}>`)).not.toContain(`<${INSTRUCTIONS_TAG}>`)
  })

  it('draws the card under an interview’s answer, with the words to change and Save', () => {
    const html = thread(interviewPrompt('Ada', 'Research & Briefs'), answer(`Thanks, that is enough.\n\n<${INSTRUCTIONS_TAG}>\n${WRITTEN}\n</${INSTRUCTIONS_TAG}>`))
    expect(html).toContain('Ada wrote its own instructions from your answers.')
    expect(html).toContain('Save as Ada’s instructions')
    expect(html).toContain('Answer in three bullet points at most.')
    // The person's own ask names the block; the teammate's answer shows the words, not its tags.
    expect(html.slice(html.indexOf('lc-agentline'))).not.toContain(`&lt;${INSTRUCTIONS_TAG}&gt;`)
    // A teammate with instructions already is told these replace them.
    expect(thread(interviewPrompt('Ada', 'Research & Briefs'), answer(`<${INSTRUCTIONS_TAG}>${WRITTEN} More.</${INSTRUCTIONS_TAG}>`), { self: { ...ADA, instructions: 'Be brief.' } as PublicTeammate }))
      .toContain('They take the place of the instructions Ada has now.')
  })

  it('draws no card outside an interview, nor while the turn is still going', () => {
    const block = answer(`<${INSTRUCTIONS_TAG}>${WRITTEN}</${INSTRUCTIONS_TAG}>`)
    expect(thread('Summarize the folder', block)).not.toContain('wrote its own instructions')
    expect(thread(interviewPrompt('Ada', 'Research & Briefs'), block, { running: true })).not.toContain('wrote its own instructions')
    // Saved already: the card says so instead of offering it again.
    expect(thread(interviewPrompt('Ada', 'Research & Briefs'), block, { self: { ...ADA, instructions: WRITTEN } as PublicTeammate })).toContain('Saved as Ada’s instructions.')
  })

  it('starts the interview in Ask for its first turn, as a review starts, so it changes nothing', () => {
    const app = readFileSync(fileURLToPath(new URL('./App.tsx', import.meta.url)), 'utf8')
    const at = app.indexOf('void startMission(interviewPrompt(')
    expect(at).toBeGreaterThan(-1)
    expect(app.indexOf('void startMission(interviewPrompt(', at + 1)).toBe(-1)
    expect(app.slice(at, app.indexOf('\n', at))).toContain('{ as: asReviewer(startAs(teammate, ')
  })

  it('refuses instructions that hold a key, before anything is saved', () => {
    expect(instructionsRefusal('Ada', 'Use the key sk-ant-abcdefghijklmnopqrstuvwxyz0123 for the API.')).toBe('They hold an Anthropic API key, and Ada’s model is given them on every turn, so they were not saved. Take it out first.') // public-guard: allow (a made-up key in a test)
    expect(instructionsRefusal('Ada', 'The database password is hunter2hunter2.')).toContain('They hold a password')
    expect(instructionsRefusal('Ada', WRITTEN)).toBeUndefined()
  })
})
