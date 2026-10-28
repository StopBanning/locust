import { readFileSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, it } from 'vitest'

import type { CodexMissionStartResponse, CodexMissionUpdate, PublicRoutine, TeammateRoute } from '../shared/ipc.js'
import { routineOkAlert } from './phone-alerts.js'
import { parseRoutineFile, routineExportText, routineFileText, routineToFile } from './routine-file.js'
import { createRoutineRunner } from './routine-runner.js'
import type { RoutineRunnerOptions } from './routine-runner.js'
import { createRoutineStore } from './routine-store.js'
import type { MissionPeerContext } from './workroom-briefing.js'

/**
 * A STEP THAT WAITS FOR YOUR OK (0.840, the OpenBot scrub's ninth take: approval steps in routines). Step 2 on can be
 * set to Wait for my OK first: the run holds before it, as a step for a busy teammate is held, the desktop, phone and
 * chat apps are told, and Go ahead on the routine's card starts it with what the step before answered.
 */
const WREN: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
const ROUTE: TeammateRoute = { runtime: 'cursor', model: 'composer-2.5', mode: 'ask' }

const routine = (overrides: Partial<PublicRoutine> = {}): PublicRoutine => ({
  routineId: 'rt_ship',
  name: 'Ship the fix',
  teammateId: 'tm_wren',
  route: ROUTE,
  steps: ['Fix the failing test in cart.ts.', 'Push the branch and open a pull request.'],
  handOffs: [{}, { askFirst: true }],
  learnedFrom: [],
  createdAt: '2026-10-28T00:00:00.000Z',
  runs: 0,
  ...overrides
})

function harness(routines: readonly PublicRoutine[]) {
  const starts: Parameters<RoutineRunnerOptions['start']>[0][] = []
  const updates: CodexMissionUpdate[] = []
  const asked: [string | undefined, string, number][] = []
  const kept: string[] = []
  const phases = new Map<string, 'completed' | 'failed' | 'cancelled' | 'interrupted'>()
  const replies = new Map<string, string>()
  const runs: string[] = []
  let counter = 0
  const held = new Map(routines.map((entry) => [entry.routineId, entry]))
  const options: RoutineRunnerOptions = {
    workspaceId: 'ws_test',
    routines: {
      get: async (id) => held.get(String(id)),
      list: async () => [...held.values()],
      recordRun: async (id) => {
        runs.push(String(id))
        const { execution: _execution, ...rest } = held.get(String(id))!
        held.set(String(id), { ...rest, runs: rest.runs + 1 })
      },
      saveProgress: async (id, execution) => { held.set(id, { ...held.get(id)!, execution }) },
      clearProgress: async (id) => {
        const { execution: _execution, ...rest } = held.get(id)!
        held.set(id, rest)
      },
      abandon: async () => undefined,
      keepSchedule: async (id) => {
        kept.push(id)
        const { execution: _execution, ...rest } = held.get(id)!
        held.set(id, rest)
      }
    },
    peerContextFor: async (teammateId) => (teammateId === 'tm_wren' ? WREN : undefined),
    routeOf: async () => ROUTE,
    replyOf: async (missionId) => replies.get(missionId),
    teammateBusy: async () => false,
    start: async (request) => {
      starts.push(request)
      counter += 1
      return { ok: true, data: { runId: `run_${String(counter)}`, missionId: `mission_${String(counter)}`, runtime: request.runtime, sandbox: 'read-only', model: request.model } } as unknown as CodexMissionStartResponse
    },
    assignOwner: async () => undefined,
    phaseOf: async (missionId) => phases.get(missionId),
    askedAQuestion: async () => false,
    notify: (update) => { updates.push(update) },
    asksFirst: (name, routineName, step) => { asked.push([name, routineName, step]) }
  }
  const finish = async (runner: ReturnType<typeof createRoutineRunner>, missionId: string, reply: string): Promise<void> => {
    phases.set(missionId, 'completed')
    replies.set(missionId, reply)
    await runner.onRunEnded({ missionId })
  }
  const notices = (): string[] => updates.flatMap((update) => (update.kind === 'relay-notice' ? [update.message] : []))
  return { starts, asked, kept, runs, held, options, finish, notices }
}

const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe('a routine step that waits for your OK', () => {
  it('holds before the step, tells the person, and Go ahead starts it with the answer of the step before', async () => {
    const h = harness([routine()])
    const runner = createRoutineRunner(h.options)
    await runner.run('rt_ship')
    await h.finish(runner, 'mission_1', 'Fixed: the total was summed twice.')
    expect(h.starts).toHaveLength(1)
    const waiting = h.held.get('rt_ship')!.execution!
    expect(waiting).toMatchObject({ status: 'held', step: 1, of: 2, canContinue: true, settledAtDispatch: true, waitsForOk: true })
    expect(waiting.reason).toBe('Step 2 waits for your OK. Read what step 1 did, then go ahead or stop here.')
    expect(h.asked).toEqual([['Wren', 'Ship the fix', 2]])
    expect(h.notices().at(-1)).toBe('Routine "Ship the fix" is waiting for your OK after step 1 of 2. Go ahead with step 2 under Routines.')
    // A restart's reconcile leaves a wait the routine asked for alone.
    await runner.reconcile()
    expect(h.held.get('rt_ship')!.execution).toMatchObject({ status: 'held', waitsForOk: true })

    expect(await runner.recover({ routineId: 'rt_ship', attemptId: waiting.attemptId, step: 1, decision: 'continue' })).toEqual({ ok: true })
    expect(h.starts).toHaveLength(2)
    expect(h.starts[1]).toMatchObject({ prompt: 'Push the branch and open a pull request.', followUpOf: 'mission_1', peer: WREN })
    // The step it started is running, not still waiting.
    expect(h.held.get('rt_ship')!.execution).toMatchObject({ status: 'running', step: 2 })
    expect(h.held.get('rt_ship')!.execution!.waitsForOk).toBeUndefined()
    await h.finish(runner, 'mission_2', 'Opened the pull request.')
    expect(h.runs).toEqual(['rt_ship'])
  })

  it('Stop here ends the run and keeps the routine as it was', async () => {
    const h = harness([routine()])
    const runner = createRoutineRunner(h.options)
    await runner.run('rt_ship')
    await h.finish(runner, 'mission_1', 'Fixed.')
    const waiting = h.held.get('rt_ship')!.execution!
    expect(await runner.recover({ routineId: 'rt_ship', attemptId: waiting.attemptId, step: 1, decision: 'keep' })).toEqual({ ok: true })
    expect(h.kept).toEqual(['rt_ship'])
    expect(h.starts).toHaveLength(1)
  })

  it('does not wait when no step asks, or when the step that asks is not the next one', async () => {
    const h = harness([routine({ steps: ['One.', 'Two.', 'Three.'], handOffs: [{}, {}, { askFirst: true }] })])
    const runner = createRoutineRunner(h.options)
    await runner.run('rt_ship')
    await h.finish(runner, 'mission_1', 'Done one.')
    expect(h.starts).toHaveLength(2)
    expect(h.asked).toEqual([])
    await h.finish(runner, 'mission_2', 'Done two.')
    expect(h.starts).toHaveLength(2)
    expect(h.asked).toEqual([['Wren', 'Ship the fix', 3]])
  })

  it('is kept by the store for one teammate too, from step 2 on: step 1 never waits', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'locust-routines-'))
    roots.push(directory)
    const store = createRoutineStore({ rootDirectory: directory })
    const saved = await store.create({ name: 'Ship the fix', teammateId: 'tm_wren', route: ROUTE, learnedFrom: [],
      steps: ['One.', 'Two.', 'Three.'], handOffs: [{ askFirst: true }, {}, { askFirst: true }] })
    expect(saved.handOffs).toEqual([{}, {}, { askFirst: true }])
    // Turned off again: nothing left to keep.
    const cleared = await store.update({ routineId: saved.routineId, name: 'Ship the fix', steps: ['One.', 'Two.', 'Three.'], handOffs: [{}, {}, {}] })
    expect(cleared.handOffs).toBeUndefined()
    await expect(store.create({ name: 'Odd', teammateId: 'tm_wren', route: ROUTE, learnedFrom: [], steps: ['One.', 'Two.'], handOffs: [{}, { askFirst: 'yes' }] }))
      .rejects.toThrow('Who takes each step does not line up with the steps')
  })

  it('travels in the routine file, as Markdown, and step 1 asking is refused on the way in', () => {
    const file = routineToFile(routine(), { steps: routine().steps, inputs: [], roleOf: () => undefined, connectorNames: [] })
    expect(file.handOffs).toEqual([{}, { askFirst: true }])
    const exported = routineExportText(file)
    expect(exported.markdown).toBe(true)
    expect(exported.text).toContain('## Step 2 · waits for your OK\n')
    const back = parseRoutineFile(exported.text)
    expect(back.ok && back.file.handOffs).toEqual([{}, { askFirst: true }])
    const json = parseRoutineFile(routineFileText({ ...file, handOffs: [{ askFirst: true }, {}] }))
    expect(json).toEqual({ ok: false, message: 'Step 1 cannot wait for your OK: it starts when you run the routine.' })
    const markdown = parseRoutineFile(exported.text.replace('## Step 1\n', '## Step 1 · waits for your OK\n'))
    expect(markdown).toEqual({ ok: false, message: 'Step 1 cannot wait for your OK: it starts when you run the routine.' })
  })

  it('still waits after an import, a starter, a team card, and is listed to other apps', () => {
    const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
    const io = read('./routine-io.ts')
    expect(io).toContain('handOffAsks: file.handOffs.map((entry) => entry.askFirst === true)')
    expect(io).toContain('handOffs: held.file.handOffs.map((entry) => (entry.askFirst === true ? { askFirst: true as const } : {}))')
    expect(read('./index.ts')).toContain('handOffs: file.handOffs.map((entry) => (entry.askFirst === true ? { askFirst: true as const } : {}))')
    expect(read('../renderer/src/routines.ts')).toContain("preview.handOffAsks?.[at] === true ? { ...entry, askFirst: true as const } : entry")
    expect(read('../renderer/src/components/RoutineFileDialog.tsx')).toContain('Waits for your OK before it starts')
    expect(read('./locust-mcp-tools.ts')).toContain('{ waits_for_your_ok: true }')
  })

  it("buzzes the phone as pressing, naming who but never the routine or the step's words", () => {
    const alert = routineOkAlert('Wren')
    expect(alert).toEqual({ kind: 'approval', title: 'Wren waits for your OK', body: 'A routine’s next step starts when you go ahead.', tag: 'waiting' })
    expect(routineOkAlert(undefined).title).toBe('A teammate waits for your OK')
  })
})
