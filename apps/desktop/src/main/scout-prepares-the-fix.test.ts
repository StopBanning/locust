import { execFile, spawn } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, it } from 'vitest'

import { inFixCopy, publishesFromCopy, stoppedBecause } from '../../resources/locust-command-guard.mjs'
import { createScout, type ScoutDeps } from './scout.js'
import { createScoutFixes, newFixId, SCOUT_FIX_TURNS, type FixTurnEnd } from './scout-fix.js'

/*
 * SCOUT PREPARES THE FIX (0.828). The fix is made in a copy that has no remote and from which the guard lets nothing
 * push; the limits stop a run and say why; Apply lands beside the person's own edits.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

const git = (args: readonly string[], cwd: string): Promise<{ code: number; out: string }> => new Promise((resolve) => {
  execFile('git', [...args], { cwd, windowsHide: true, env: { ...process.env, GIT_TERMINAL_PROMPT: '0' } }, (error, stdout, stderr) => {
    resolve({ code: error === null ? 0 : typeof error.code === 'number' ? error.code : 1, out: `${String(stdout)}${String(stderr)}` })
  })
})

/** A project with a remote and a check that passes once `answer.txt` says 42. */
async function project() {
  const top = await mkdtemp(join(tmpdir(), 'locust-scout-fix-'))
  roots.push(top)
  const folder = join(top, 'project')
  await mkdir(folder, { recursive: true })
  await writeFile(join(folder, 'answer.txt'), '41\n', 'utf8')
  await writeFile(join(folder, 'notes.md'), 'mine\n', 'utf8')
  await git(['init', '-q'], folder)
  await git(['remote', 'add', 'origin', 'https://github.com/acme/app.git'], folder)
  return { top, folder, root: join(top, 'scout-fixes') }
}

const checkOf = async (_command: string, cwd: string) => {
  const answer = (await readFile(join(cwd, 'answer.txt'), 'utf8').catch(() => '')).trim()
  return answer === '42' ? { passed: true, lines: [] } : { passed: false, lines: [`expected 42, got ${answer}`] }
}

/** A teammate that, each turn, does what `work` says in the copy and ends. */
function fixesWith(root: string, work: (cwd: string, turn: number) => Promise<FixTurnEnd | 'hang'>, more: Partial<Parameters<typeof createScoutFixes>[0]> = {}) {
  const started: { prompt: string; cwd: string; followUpOf?: string }[] = []
  const stopped: string[] = []
  const ends = new Map<string, FixTurnEnd>()
  let timers: (() => void)[] = []
  const fixes = createScoutFixes({
    root,
    runCheck: checkOf,
    start: async ({ prompt, cwd, followUpOf }) => {
      started.push({ prompt, cwd, ...(followUpOf === undefined ? {} : { followUpOf }) })
      const missionId = `mission_${String(started.length)}`
      const turn = started.length
      void (async () => {
        const end = await work(cwd, turn)
        if (end === 'hang') return
        ends.set(missionId, end)
        await fixes.onRunEnded({ missionId })
      })()
      return { missionId, runId: `run_${String(turn)}` }
    },
    stop: (runId) => { stopped.push(runId) },
    endOf: async (missionId) => ends.get(missionId) ?? { phase: 'cancelled' },
    later: (task) => { timers.push(task); return task },
    cancel: (handle) => { timers = timers.filter((one) => one !== handle) },
    ...more
  })
  return { fixes, started, stopped, runOutOfTime: () => { for (const task of timers.splice(0)) task() } }
}

describe('Scout prepares the fix', () => {
  it('works in a copy with no remote, where a push fails and the guard refuses push, remote and gh', async () => {
    const { folder, root } = await project()
    let copy = ''
    const h = fixesWith(root, async (cwd) => {
      copy = cwd
      await writeFile(join(cwd, 'answer.txt'), '42\n', 'utf8')
      return { phase: 'completed', said: 'Changed the answer to 42.' }
    })
    const fixId = newFixId()
    const outcome = await h.fixes.prepare({ fixId, teammateId: 'tm_wren', name: 'Wren', command: 'npm test', folder })
    expect(outcome).toMatchObject({ kind: 'ready', files: ['answer.txt'], missionId: 'mission_1' })
    expect(copy).toBe(join(root, fixId))
    // MEASURED: the copy is its own repository, with no remote, and a push from it fails.
    expect((await git(['remote'], copy)).out.trim()).toBe('')
    expect((await git(['rev-parse', '--show-toplevel'], copy)).out.trim().replace(/\\/g, '/').toLowerCase()).toBe(copy.replace(/\\/g, '/').toLowerCase())
    const push = await git(['push', 'origin', 'HEAD'], copy)
    expect(push.code).not.toBe(0)
    // The guard knows the copy by its folder, and refuses what would publish from it -- and only there.
    expect(inFixCopy([copy])).toBe(true)
    expect(inFixCopy([folder])).toBe(false)
    for (const command of ['git push origin main', 'git -C . push', 'git remote add origin https://github.com/acme/app.git', 'gh pr create --fill', 'npm test && gh api repos/acme/app', '"C:\\Program Files\\GitHub CLI\\gh.exe" pr create']) {
      expect(publishesFromCopy(command), command).toBe(true)
      expect(stoppedBecause(command, () => undefined, [copy]), command).toMatch(/^Refused: this is a copy Scout made to prepare a fix/)
      expect(stoppedBecause(command, () => undefined, [folder]), command).toBeUndefined()
    }
    for (const command of ['npm test', 'git status', 'git diff', 'git log --grep push', 'cat ghost.txt']) expect(stoppedBecause(command, () => undefined, [copy]), command).toBeUndefined()
  }, 30_000)

  it('refuses a push in the copy through the hook itself, from the folder it runs in', async () => {
    const { root } = await project()
    const copy = join(root, newFixId())
    await mkdir(copy, { recursive: true })
    const guard = fileURLToPath(new URL('../../resources/locust-command-guard.mjs', import.meta.url))
    const ask = (cwd: string, event: object): Promise<string> => new Promise((resolve) => {
      const child = spawn(process.execPath, [guard], { cwd, stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true })
      let out = ''
      child.stdout.on('data', (chunk) => { out += String(chunk) })
      child.on('close', () => { resolve(out) })
      child.stdin.end(JSON.stringify(event))
    })
    const event = { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'git push origin main' } }
    expect(JSON.parse(await ask(copy, event)).hookSpecificOutput.permissionDecision).toBe('deny')
    // As the runtime names it, from anywhere.
    expect(JSON.parse(await ask(tmpdir(), { ...event, cwd: copy })).hookSpecificOutput.permissionDecision).toBe('deny')
    expect(await ask(tmpdir(), event)).toBe('')
  })

  it('stops at the turn limit and the time limit, and says why; a turn that changes nothing says what it said', async () => {
    const { folder, root } = await project()
    const stubborn = fixesWith(root, async (cwd, turn) => {
      await writeFile(join(cwd, 'answer.txt'), `${String(40 - turn)}\n`, 'utf8')
      return { phase: 'completed', said: 'Tried.' }
    })
    const tries = await stubborn.fixes.prepare({ fixId: newFixId(), teammateId: 'tm_wren', name: 'Wren', command: 'npm test', folder })
    expect(tries).toEqual({ kind: 'unfixed', why: `The check still fails after ${String(SCOUT_FIX_TURNS)} tries: expected 42, got ${String(40 - SCOUT_FIX_TURNS)}`, missionId: `mission_${String(SCOUT_FIX_TURNS)}` })
    expect(stubborn.started).toHaveLength(SCOUT_FIX_TURNS)
    expect(stubborn.started[1]).toMatchObject({ followUpOf: 'mission_1' })
    expect(stubborn.started[1]?.prompt).toContain('still fails after your change (try 2 of 2)')

    const slow = fixesWith(root, async () => 'hang')
    const waiting = slow.fixes.prepare({ fixId: newFixId(), teammateId: 'tm_wren', name: 'Wren', command: 'npm test', folder })
    // Once its turn has started and is waited on, the time runs out.
    while (slow.started.length === 0) await new Promise((resolve) => setTimeout(resolve, 10))
    await new Promise((resolve) => setTimeout(resolve, 10))
    slow.runOutOfTime()
    expect(await waiting).toEqual({ kind: 'unfixed', why: 'It ran past 30 minutes, so Locust stopped it.', missionId: 'mission_1' })
    expect(slow.stopped).toEqual(['run_1'])

    const idle = fixesWith(root, async () => ({ phase: 'completed', said: 'The test needs a database I cannot reach.' }))
    expect(await idle.fixes.prepare({ fixId: newFixId(), teammateId: 'tm_wren', name: 'Wren', command: 'npm test', folder }))
      .toEqual({ kind: 'unfixed', why: 'Wren changed nothing, and said: “The test needs a database I cannot reach.”', missionId: 'mission_1' })
    // The first prompt says where it is and what it may not do.
    expect(idle.started[0]?.prompt).toContain('You are working in a copy of their folder')
    expect(idle.started[0]?.prompt).toContain('Do not push, add a remote, or use gh')
    expect(idle.started[0]?.prompt).toContain('    expected 42, got 41')
  }, 30_000)

  it('applies beside the person’s own edits, and refuses when they changed the same file since', async () => {
    const { folder, root } = await project()
    const h = fixesWith(root, async (cwd) => {
      await writeFile(join(cwd, 'answer.txt'), '42\n', 'utf8')
      return { phase: 'completed', said: 'Fixed.' }
    })
    const first = newFixId()
    expect(await h.fixes.prepare({ fixId: first, teammateId: 'tm_wren', name: 'Wren', command: 'npm test', folder })).toMatchObject({ kind: 'ready' })
    // The person keeps working on another file meanwhile.
    await writeFile(join(folder, 'notes.md'), 'mine, changed while the fix was made\n', 'utf8')
    expect(await h.fixes.apply(first)).toEqual({ kind: 'brought', files: ['answer.txt'] })
    expect(await readFile(join(folder, 'answer.txt'), 'utf8')).toBe('42\n')
    expect(await readFile(join(folder, 'notes.md'), 'utf8')).toBe('mine, changed while the fix was made\n')
    expect(await h.fixes.apply(first)).toEqual({ kind: 'gone' })

    await writeFile(join(folder, 'answer.txt'), '41\n', 'utf8')
    const second = newFixId()
    expect(await h.fixes.prepare({ fixId: second, teammateId: 'tm_wren', name: 'Wren', command: 'npm test', folder })).toMatchObject({ kind: 'ready' })
    await writeFile(join(folder, 'answer.txt'), '40\n', 'utf8')
    expect(await h.fixes.apply(second)).toEqual({ kind: 'your-changes', files: ['answer.txt'] })
    expect(await readFile(join(folder, 'answer.txt'), 'utf8')).toBe('40\n')
  })
})

/* The card: off until turned on, the day's limit, and each step said. */
async function scoutHarness(overrides: Partial<ScoutDeps> = {}) {
  const top = await mkdtemp(join(tmpdir(), 'locust-scout-fixcard-'))
  roots.push(top)
  let checkPasses = false
  let mark = 1
  const prepared: string[] = []
  const stops: string[] = []
  let finish: ((outcome: { kind: 'ready'; files: readonly string[]; missionId: string }) => void) | undefined
  const teammates = new Set<string>()
  const deps: ScoutDeps = {
    file: join(top, 'scout.json'),
    now: () => new Date(2026, 9, 23, 11, 0, 0),
    github: async () => ({ prs: [], reviews: [] }),
    waiting: async () => [],
    failedTurn: async () => undefined,
    team: async () => [{ teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }],
    ask: async () => ({ refused: 'No model in this test.' }),
    stop: () => undefined,
    nextFreeModel: async () => undefined,
    answerOf: async () => ({ failed: 'It said nothing.' }),
    makeTeammate: async () => { teammates.add('tm_scout_1'); return 'tm_scout_1' },
    teammateExists: async (id) => teammates.has(id),
    changed: () => undefined,
    notify: () => undefined,
    schedule: () => 'timer',
    clear: () => undefined,
    later: () => 'later',
    cancel: () => undefined,
    folderMark: async () => `mark-${String(mark)}`,
    runCheck: async () => ({ command: 'npm test', passed: checkPasses, lines: checkPasses ? [] : ['1 test failed'] }),
    fixes: {
      prepare: (input) => { prepared.push(input.fixId); return new Promise((resolve) => { finish = resolve }) },
      apply: async () => ({ kind: 'brought', files: ['answer.txt'] }),
      discard: async () => undefined,
      stop: (why) => { stops.push(why) }
    },
    ...overrides
  }
  return {
    scout: createScout(deps), prepared, stops,
    finish: (files: readonly string[]) => { finish?.({ kind: 'ready', files, missionId: 'mission_fix' }) },
    fail: () => { checkPasses = false; mark += 1 },
    pass: () => { checkPasses = true; mark += 1 }
  }
}
const ROUTE = { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' }
const settle = () => new Promise((resolve) => setTimeout(resolve, 20))
const checkCard = async (scout: Awaited<ReturnType<typeof scoutHarness>>['scout']) => (await scout.state()).cards.find((card) => card.key.endsWith(':fails'))

describe('the card of a fix Scout prepared', () => {
  it('prepares nothing until fixes are on; then says who is preparing it, then that it is ready, and Apply brings it in', async () => {
    const h = await scoutHarness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ watches: { github: true, teammates: true, comments: true, check: true } })
    await h.scout.look()
    expect((await checkCard(h.scout))?.fix).toBeUndefined()
    expect(h.prepared).toEqual([])

    await h.scout.set({ fixes: { on: true, teammateId: 'tm_wren', perDay: 2 } })
    h.pass(); await h.scout.look()
    h.fail(); await h.scout.look()
    await settle()
    const card = await checkCard(h.scout)
    expect(card?.fix).toMatchObject({ state: 'preparing', name: 'Wren' })
    expect((await h.scout.state()).today?.fixes).toBe(1)
    h.finish(['answer.txt', 'test/answer.test.ts'])
    await settle()
    expect((await checkCard(h.scout))?.fix).toMatchObject({ state: 'ready', files: ['answer.txt', 'test/answer.test.ts'], missionId: 'mission_fix' })
    await h.scout.applyFix(card!.cardId)
    expect((await checkCard(h.scout))?.fix).toEqual({ state: 'applied', files: ['answer.txt'] })
  })

  it('stops at the day’s limit and says why; turned off, the fix under way stops', async () => {
    const h = await scoutHarness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ watches: { github: true, teammates: true, comments: true, check: true }, fixes: { on: true, teammateId: 'tm_wren', perDay: 1 } })
    await h.scout.look()
    await settle()
    expect(h.prepared).toHaveLength(1)
    h.finish(['answer.txt'])
    await settle()
    h.pass(); await h.scout.look()
    h.fail(); await h.scout.look()
    await settle()
    expect(h.prepared).toHaveLength(1)
    expect((await checkCard(h.scout))?.fix).toEqual({ state: 'held', why: 'Today’s fix is used, so Scout did not prepare one. It can again tomorrow, or raise the limit in Settings > General, under Scout.' })

    const off = await scoutHarness()
    await off.scout.turnOn(ROUTE)
    await off.scout.set({ watches: { github: true, teammates: true, comments: true, check: true }, fixes: { on: true, teammateId: 'tm_wren', perDay: 2 } })
    await off.scout.look()
    await settle()
    await off.scout.set({ on: false })
    expect(off.stops).toEqual(['Scout was turned off, so the fix stopped.'])
  })

  it('says when nobody is picked to prepare it', async () => {
    const h = await scoutHarness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ watches: { github: true, teammates: true, comments: true, check: true }, fixes: { on: true, perDay: 2 } })
    await h.scout.look()
    await settle()
    expect((await checkCard(h.scout))?.fix).toEqual({ state: 'held', why: 'No teammate is picked to prepare fixes, so Scout did not prepare one. Pick one in Settings > General, under Scout.' })
  })
})
