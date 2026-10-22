import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import type { FolderPullRequest } from '../shared/pull-request.js'
import { MAX_SCOUT_NOTES, notesFrom, scoutNoteOf, scoutPrompt } from '../shared/scout.js'
import { createScout, type ScoutDeps } from './scout.js'

/*
 * SCOUT KEEPS NOTES YOU CAN READ (0.816). The deep dive's most-cited complaint about dots: memory nobody can see or
 * fix. Scout's notes are plain lines, read into every look; its model may add one a look, which Locust dates and keeps
 * once; the person changes or clears them in Settings, and what they keep is what the next look reads.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

const failing: FolderPullRequest = { number: 212, title: 'Talk starts from any app', url: 'https://github.com/acme/app/pull/212', state: 'open', checks: 'failing', checkCount: 9, notPassed: 2 }

async function harness(overrides: Partial<ScoutDeps> = {}) {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-notes-'))
  roots.push(root)
  let prs: FolderPullRequest[] = []
  const asked: string[] = []
  const answers = new Map<string, { text: string } | { failed: string }>()
  const teammates = new Set<string>()
  const deps: ScoutDeps = {
    file: join(root, 'scout.json'),
    now: () => new Date(2026, 9, 22, 14, 0, 0),
    github: async () => ({ prs, reviews: [] }),
    waiting: async () => [],
    failedTurn: async () => undefined,
    team: async () => [{ teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }],
    ask: async (_teammateId, prompt) => { asked.push(prompt); return { missionId: `look_${String(asked.length)}`, runId: `run_${String(asked.length)}` } },
    stop: () => undefined,
    nextFreeModel: async () => undefined,
    answerOf: async (missionId) => answers.get(missionId) ?? { failed: 'It said nothing.' },
    makeTeammate: async () => { teammates.add('tm_scout_1'); return 'tm_scout_1' },
    teammateExists: async (id) => teammates.has(id),
    changed: () => undefined,
    notify: () => undefined,
    schedule: () => 'timer',
    clear: () => undefined,
    later: () => 'later',
    cancel: () => undefined,
    ...overrides
  }
  return { scout: createScout(deps), deps, asked, answers, setPrs: (next: FolderPullRequest[]) => { prs = next } }
}
const ROUTE = { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' }

describe('Scout’s notes', () => {
  it('are read as plain lines, list marks and blank lines dropped, the newest kept', () => {
    expect(notesFrom('- Wren fixes CI fastest\n\n* The person merges on Fridays  \n')).toEqual(['Wren fixes CI fastest', 'The person merges on Fridays'])
    expect(notesFrom(Array.from({ length: 50 }, (_, index) => `note ${String(index)}`).join('\n'))).toHaveLength(MAX_SCOUT_NOTES)
    expect(scoutNoteOf('ITEM 1\nTAKE: x\nNOTE: Wren owns the Mac build.')).toBe('Wren owns the Mac build.')
    expect(scoutNoteOf('ITEM 1\nTAKE: x\n**NOTE:** none')).toBeUndefined()
    expect(scoutNoteOf('ITEM 1\nTAKE: nothing to keep')).toBeUndefined()
  })

  it('are read into a look, which may add one, dated, said once', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.saveNotes('The person merges on Fridays')
    h.setPrs([failing])
    await h.scout.look()
    expect(h.asked[0]).toContain('Your notes from earlier looks, which the person can read and change:\n- The person merges on Fridays')
    expect(h.asked[0]).toContain('add one line: NOTE: <one sentence>')
    h.answers.set('look_1', { text: 'ITEM 1\nTAKE: The Mac check again.\nHAND TO: Wren\nDRAFT: Fix the Mac timeout.\nNOTE: Wren owns the Mac build.' })
    await h.scout.onRunEnded({ missionId: 'look_1' })
    expect((await h.scout.state()).notes).toEqual(['The person merges on Fridays', '2026-10-22: Wren owns the Mac build.'])
    // The same note again is not kept twice.
    h.setPrs([{ ...failing, notPassed: 3 }])
    await h.scout.look()
    h.answers.set('look_2', { text: 'ITEM 1\nTAKE: Still failing.\nNOTE: Wren owns the Mac build.' })
    await h.scout.onRunEnded({ missionId: 'look_2' })
    expect((await h.scout.state()).notes).toHaveLength(2)
  })

  it('are what the person keeps: changed or cleared in Settings, and kept on disk', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.saveNotes('- one\n- two')
    expect((await h.scout.state()).notes).toEqual(['one', 'two'])
    const onDisk = JSON.parse(await readFile(h.deps.file, 'utf8')) as { notes?: string[] }
    expect(onDisk.notes).toEqual(['one', 'two'])
    await h.scout.saveNotes('')
    expect(await h.scout.state()).not.toHaveProperty('notes')
    // Read back by a new Scout on the same file.
    await h.scout.saveNotes('kept')
    const again = createScout(h.deps)
    expect((await again.state()).notes).toEqual(['kept'])
  })

  it('leave a look’s prompt as it was when there are none', () => {
    expect(scoutPrompt({ facts: [{ key: 'k', urgency: 'now', said: 'A fact.' }], team: [] })).not.toContain('Your notes from earlier looks')
  })
})
