import { mkdir, mkdtemp, readFile, rm, utimes, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import type { CodexMissionStartResponse, PublicRoutine } from '../shared/ipc.js'
import { validSchedule, watchedFolderValid } from '../shared/routine-schedule.js'
import { MAX_FIRES_PER_HOUR, RELEASE_MS, SETTLE_MS, arrivalNote, createFileArrivals, watchedPath } from './routine-file-watch.js'
import { createRoutineRunner } from './routine-runner.js'
import type { RoutineRunnerOptions } from './routine-runner.js'

/**
 * A FILE ARRIVING STARTS ITS ROUTINE (0.522, folder watchers). The baseline
 * starts nothing; a new file counts once it stops changing; at most six runs
 * an hour; files together are one run; step 1 is told which.
 */
const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})
async function project(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'locust-watch-'))
  roots.push(root)
  await mkdir(join(root, 'inbox'))
  return root
}
const T0 = 1_000_000

describe('a watched folder', () => {
  it('starts nothing for what is there at the first look, and counts a new file once it stops changing', async () => {
    const root = await project()
    await writeFile(join(root, 'inbox', 'old.csv'), 'old\n')
    const arrivals = createFileArrivals({ projectFolder: root })
    const watching = [{ routineId: 'rt_1', folder: 'inbox' }]
    await arrivals.poll(watching, T0)
    expect(arrivals.ready('rt_1', T0)).toEqual([])
    await writeFile(join(root, 'inbox', 'new.csv'), 'new\n')
    await arrivals.poll(watching, T0 + 1_000)
    expect(arrivals.ready('rt_1', T0 + 1_000)).toEqual([])
    // Still changing: the clock of settling starts again.
    await writeFile(join(root, 'inbox', 'new.csv'), 'new, longer\n')
    await arrivals.poll(watching, T0 + SETTLE_MS)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS)).toEqual([])
    await arrivals.poll(watching, T0 + SETTLE_MS * 2 + 1)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS * 2 + 1)).toEqual(['new.csv'])
  })

  it('skips hidden, temporary and partly downloaded names, and folders', async () => {
    const root = await project()
    const arrivals = createFileArrivals({ projectFolder: root })
    const watching = [{ routineId: 'rt_1', folder: 'inbox' }]
    await arrivals.poll(watching, T0)
    for (const name of ['.DS_Store', '~$budget.xlsx', 'a.tmp', 'b.crdownload', 'c.part']) await writeFile(join(root, 'inbox', name), 'x')
    await mkdir(join(root, 'inbox', 'sub'))
    await arrivals.poll(watching, T0 + 1)
    await arrivals.poll(watching, T0 + SETTLE_MS + 2)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS + 2)).toEqual([])
  })

  it('runs at most six times an hour; files waiting past that wait for the hour', async () => {
    const root = await project()
    const arrivals = createFileArrivals({ projectFolder: root })
    const watching = [{ routineId: 'rt_1', folder: 'inbox' }]
    await arrivals.poll(watching, T0)
    let now = T0
    for (let index = 0; index < MAX_FIRES_PER_HOUR + 1; index += 1) {
      const path = join(root, 'inbox', `f${String(index)}.txt`)
      await writeFile(path, String(index))
      await utimes(path, new Date(now), new Date(now))
      await arrivals.poll(watching, now + 1)
      await arrivals.poll(watching, now + SETTLE_MS + 2)
      now += SETTLE_MS + 3
      const ready = arrivals.ready('rt_1', now)
      if (index < MAX_FIRES_PER_HOUR) {
        expect(ready).toEqual([`f${String(index)}.txt`])
        arrivals.fired('rt_1', ready, now)
      } else {
        expect(ready).toEqual([])
      }
    }
    expect(arrivals.ready('rt_1', now + 3_600_000)).toEqual([`f${String(MAX_FIRES_PER_HOUR)}.txt`])
  })

  it('is a folder inside the project, by name -- never outside it', async () => {
    expect(watchedFolderValid('inbox')).toBe(true)
    expect(watchedFolderValid('docs/incoming')).toBe(true)
    for (const bad of ['', '..', '../x', 'a/../../x', 'C:\\Windows', '/etc', 'in:box', '.']) expect(watchedFolderValid(bad), bad).toBe(false)
    const root = await project()
    expect(watchedPath(root, 'inbox')).toBe(join(root, 'inbox'))
    expect(watchedPath(root, '../other')).toBeUndefined()
    expect(watchedPath(root, '.')).toBeUndefined()
    expect(validSchedule({ kind: 'files', folder: 'inbox' })).toBe(true)
    expect(validSchedule({ kind: 'files', folder: '../x' })).toBe(false)
  })
})

describe('a routine on a new file', () => {
  it('starts on the tick when files are ready, tells step 1 their names, and only then', async () => {
    const held = new Map<string, PublicRoutine>([['rt_1', {
      routineId: 'rt_1', name: 'Read new invoices', teammateId: 'tm_wren',
      route: { runtime: 'codex', model: 'account-default', mode: 'ask' },
      steps: ['Summarize it in one line.'], learnedFrom: ['mission_a'], createdAt: '2026-10-01T00:00:00.000Z', runs: 0,
      schedule: { kind: 'files', folder: 'inbox' }
    }]])
    const starts: string[] = []
    let ready: readonly string[] = []
    const fired: string[][] = []
    const options = {
      workspaceId: 'ws_test',
      routines: {
        get: async (id: unknown) => held.get(String(id)),
        list: async () => [...held.values()],
        recordRun: async () => undefined,
        saveProgress: async (id: string, execution: unknown) => { held.set(id, { ...held.get(id)!, execution } as PublicRoutine) },
        clearProgress: async () => undefined,
        abandon: async () => undefined,
        keepSchedule: async () => undefined
      },
      peerContextFor: async () => ({ self: { teammateId: 'tm_wren', name: 'Wren', role: 'Ops' }, others: [] }),
      start: async (request: { readonly prompt: string }) => {
        starts.push(request.prompt)
        return { ok: true, data: { runId: 'run_1', missionId: 'mission_1', runtime: 'codex', sandbox: 'read-only' } } as unknown as CodexMissionStartResponse
      },
      assignOwner: async () => undefined,
      phaseOf: async () => undefined,
      notify: () => undefined,
      arrivals: { ready: () => ready, fired: (_id: string, files: readonly string[]) => { fired.push([...files]) } }
    } as unknown as RoutineRunnerOptions
    const runner = createRoutineRunner(options)
    expect(await runner.tick(new Date('2026-10-01T09:00:00Z'))).toEqual([])
    ready = ['invoice-1043.pdf', 'invoice-1051.pdf']
    expect(await runner.tick(new Date('2026-10-01T09:01:00Z'))).toEqual(['rt_1'])
    expect(starts[0]).toBe(`${arrivalNote('inbox', ready)}\n\nSummarize it in one line.`)
    expect(starts[0]).toContain('2 new files just arrived: inbox/invoice-1043.pdf, inbox/invoice-1051.pdf.')
    expect(fired).toEqual([['invoice-1043.pdf', 'invoice-1051.pdf']])
  })
})

describe('a file a teammate wrote', () => {
  it('starts nothing: held while a run is live, and let go when its turn says it wrote it', async () => {
    const root = await project()
    const arrivals = createFileArrivals({ projectFolder: root })
    const watching = [{ routineId: 'rt_1', folder: 'inbox' }]
    await arrivals.poll(watching, T0)
    // "When a file lands in inbox/, write its summary there": the summary is the run's own.
    await writeFile(join(root, 'inbox', 'summary.md'), 'Three invoices.\n')
    await arrivals.poll(watching, T0 + 1, true)
    await arrivals.poll(watching, T0 + SETTLE_MS + 2, true)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS + 2)).toEqual([])
    arrivals.wrote(root, ['inbox/summary.md', 'notes/elsewhere.md'])
    await arrivals.poll(watching, T0 + SETTLE_MS + RELEASE_MS + 10, false)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS + RELEASE_MS + 10)).toEqual([])
  })

  it('is told apart from a person’s file that landed while it worked, which still starts the routine', async () => {
    const root = await project()
    const arrivals = createFileArrivals({ projectFolder: root })
    const watching = [{ routineId: 'rt_1', folder: 'inbox' }]
    await arrivals.poll(watching, T0)
    await writeFile(join(root, 'inbox', 'summary.md'), 'mine\n')
    await writeFile(join(root, 'inbox', 'invoice-1060.pdf'), 'theirs\n')
    await arrivals.poll(watching, T0 + 1, true)
    await arrivals.poll(watching, T0 + SETTLE_MS + 2, true)
    // The turn's record is written as it ends, so a moment of quiet is waited for before anything held counts.
    await arrivals.poll(watching, T0 + SETTLE_MS + 3, false)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS + 3)).toEqual([])
    arrivals.wrote(root, ['inbox/summary.md'])
    await arrivals.poll(watching, T0 + SETTLE_MS + 2 + RELEASE_MS, false)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS + 2 + RELEASE_MS)).toEqual(['invoice-1060.pdf'])
  })

  it('is let go even when its turn ends before the file has stopped changing', async () => {
    const root = await project()
    const arrivals = createFileArrivals({ projectFolder: root })
    const watching = [{ routineId: 'rt_1', folder: 'inbox' }]
    await arrivals.poll(watching, T0)
    await writeFile(join(root, 'inbox', 'report.csv'), 'a,b\n')
    arrivals.wrote(root, ['inbox/report.csv'])
    await arrivals.poll(watching, T0 + 1)
    await arrivals.poll(watching, T0 + SETTLE_MS + 2)
    expect(arrivals.ready('rt_1', T0 + SETTLE_MS + 2)).toEqual([])
  })

  it('reaches the watcher from every turn Undo records, and the host says when a run is live', async () => {
    const host = await readFile(new URL('./index.ts', import.meta.url), 'utf8')
    expect(host).toContain('void checkpoints.changed(record.workspace, record.before, record.after).then((paths) => turnWrote?.(record.workspace, paths), () => undefined)')
    expect(host).toContain('turnWrote = (workspace, paths) => fileArrivals.wrote(workspace, paths)')
    expect(host).toContain('await fileArrivals.poll(watching, Date.now(), codexMissions.liveMissionIds().length > 0 || antigravityMissions.liveMissionIds().length > 0)')
  })
})
