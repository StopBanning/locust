import { mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { createScout, type ScoutDeps } from './scout.js'

/*
 * A SCOUT FILE IT CANNOT READ IS NEVER WRITTEN OVER (0.823, Grok's review of 2026-10-22). Any failed read of
 * scout.json was an empty Scout -- off, no cards, no notes, no week -- and the next save wrote that over the file, so a
 * file busy at start was a wiped Scout. A missing file is a first start; one that does not parse is kept beside it;
 * one that cannot be read at all is left as it is, and nothing is written over it.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

async function harness(prepare: (file: string) => Promise<void>) {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-unreadable-'))
  roots.push(root)
  const file = join(root, 'scout.json')
  await prepare(file)
  const deps: ScoutDeps = {
    file,
    now: () => new Date(2026, 9, 22, 14, 0, 0),
    github: async () => ({ prs: [], reviews: [] }),
    waiting: async () => [],
    failedTurn: async () => undefined,
    team: async () => [],
    ask: async () => ({ missionId: 'look_1', runId: 'run_1' }),
    stop: () => undefined,
    nextFreeModel: async () => undefined,
    answerOf: async () => ({ failed: 'It said nothing.' }),
    makeTeammate: async () => 'tm_scout_1',
    teammateExists: async () => true,
    changed: () => undefined,
    notify: () => undefined,
    schedule: () => 'timer',
    clear: () => undefined,
    // A read tried again waits a moment: here, none.
    later: (task) => setTimeout(task, 0),
    cancel: () => undefined
  }
  return { scout: createScout(deps), root, file }
}

describe('a Scout file', () => {
  it('missing, is a first start, and the first save writes it', async () => {
    const { scout, file } = await harness(async () => undefined)
    expect((await scout.state()).settings.on).toBe(false)
    await scout.set({ goal: 'Watch the Mac build.' })
    expect(JSON.parse(await readFile(file, 'utf8')).settings.goal).toBe('Watch the Mac build.')
  })

  it('that does not parse, is kept beside it, and Scout starts fresh', async () => {
    const { scout, root, file } = await harness(async (path) => { await writeFile(path, '{"settings": {"on": true, "goal": "half a fi', 'utf8') })
    expect((await scout.state()).settings.on).toBe(false)
    const kept = (await readdir(root)).filter((name) => name.startsWith('scout.json.unreadable-'))
    expect(kept).toHaveLength(1)
    expect(await readFile(join(root, kept[0]!), 'utf8')).toBe('{"settings": {"on": true, "goal": "half a fi')
    await scout.set({ goal: 'Watch the Mac build.' })
    expect(JSON.parse(await readFile(file, 'utf8')).settings.goal).toBe('Watch the Mac build.')
  })

  it('that cannot be read at all, is left as it is: Scout works from memory and writes nothing over it', async () => {
    // A folder where the file should be: every read fails, and not because it is missing.
    const { scout, file } = await harness(async (path) => { await mkdir(path); await writeFile(join(path, 'inside.txt'), 'kept', 'utf8') })
    expect((await scout.state()).settings.on).toBe(false)
    const changed = await scout.set({ goal: 'Watch the Mac build.' })
    expect(changed.settings.goal).toBe('Watch the Mac build.')
    expect((await stat(file)).isDirectory()).toBe(true)
    expect(await readFile(join(file, 'inside.txt'), 'utf8')).toBe('kept')
  })
})
