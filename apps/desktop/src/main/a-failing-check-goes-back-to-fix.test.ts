import { describe, expect, it } from 'vitest'

import { CHECK_FIX_STOPPED, checkFixPrompt, parsedCheckFixes, parsedCheckFixTries } from '../shared/check-fix.js'
import type { CheckFixStarter } from '../shared/check-fix.js'
import { createCheckFix } from './check-fix.js'
import type { CheckedTurn, CheckFixStart } from './check-fix.js'
import type { EditCheckResult } from './edit-check.js'
import { goalFixPrompt } from './routine-runner.js'

/*
 * FIX UNTIL THE CHECK PASSES (0.812). The check after edits showed a failure with a button to send it back (Colin,
 * 2026-09-25: the button, not a loop that spends turns). Now a person can choose the loop for a folder -- OpenCode
 * hands the compiler's word straight back, Aider runs its tests, a Claude Code Stop hook keeps the model going -- and
 * it stops when the check passes, when the fixes run out, or when a fix changes nothing.
 */

interface Started {
  readonly followUpOf: string
  readonly teammateId?: string
  readonly prompt: string
  readonly startedBy: CheckFixStarter
}

function harness(input: {
  readonly most?: number
  readonly sentByPerson?: boolean
  readonly busy?: boolean
  readonly start?: (asked: Started) => Promise<CheckFixStart>
} = {}) {
  const started: Started[] = []
  const notes: { readonly missionId: string; readonly message: string; readonly level: string }[] = []
  const askedFor: (string | undefined)[] = []
  let fixes = 0
  const checkFix = createCheckFix({
    mostFixes: async (workspaceId) => {
      askedFor.push(workspaceId)
      return 'most' in input ? input.most : 2
    },
    sentByPerson: () => input.sentByPerson ?? true,
    busy: async () => input.busy ?? false,
    nameOf: async () => 'Wren',
    start: async (asked) => {
      started.push(asked)
      if (input.start !== undefined) return input.start(asked)
      fixes += 1
      return { ok: true, missionId: `fix_${String(fixes)}` }
    },
    note: (missionId, message, level) => notes.push({ missionId, message, level })
  })
  return { checkFix, started, notes, askedFor }
}

const personTurn: CheckedTurn = { missionId: 'turn_1', teammateId: 'tm_wren', completed: true, apart: false }
const fixTurn = (missionId: string, fix: number): CheckedTurn => ({ missionId, teammateId: 'tm_wren', completed: true, apart: false, startedBy: { kind: 'check-fix', fix } })

/** A failed check as edit-check.ts reports it: what is new against the check before, and all of both. */
function failed(lines: readonly string[], before?: readonly string[]): EditCheckResult {
  const fresh = before === undefined ? lines : lines.filter((line) => !before.includes(line))
  return {
    command: 'npm test',
    outcome: 'failed',
    newLines: fresh,
    unchanged: before !== undefined && fresh.length === 0,
    first: before === undefined,
    lines,
    ...(before === undefined ? {} : { before })
  }
}
const passed: EditCheckResult = { command: 'npm test', outcome: 'passed', newLines: [], unchanged: false, first: false }

describe('a failing check goes back to the teammate to fix', () => {
  it('sends what the turn broke back to the same conversation, as fix 1 of the most, recorded as Locust\'s', async () => {
    const h = harness({ most: 3 })
    await h.checkFix.onChecked(personTurn, failed(['FAIL adds two numbers', 'Tests 1 failed'], ['Tests 4 passed']))

    expect(h.started).toHaveLength(1)
    const [asked] = h.started
    expect(asked?.followUpOf).toBe('turn_1')
    expect(asked?.teammateId).toBe('tm_wren')
    expect(asked?.startedBy).toEqual({ kind: 'check-fix', fix: 1, of: 3, command: 'npm test' })
    expect(asked?.prompt).toContain('The check `npm test` failed after your last turn, with problems it did not have before (fix 1 of 3).')
    expect(asked?.prompt).toContain('    FAIL adds two numbers')
    // Never the green line from before: only what the turn broke.
    expect(asked?.prompt).not.toContain('Tests 4 passed')
    expect(asked?.prompt).toContain('Do not weaken, skip or delete tests')
    expect(h.notes).toEqual([])
  })

  it('reads the setting of the folder the turn ran in, not the folder now on screen (0.813, Sol\'s review)', async () => {
    const h = harness()
    await h.checkFix.onChecked({ ...personTurn, workspaceId: 'ws_folder_a' }, failed(['FAIL x'], []))
    expect(h.askedFor).toEqual(['ws_folder_a'])
  })

  it('does nothing while the folder is set to show the failure, or for a check that did not fail', async () => {
    const off = harness({ most: undefined })
    await off.checkFix.onChecked(personTurn, failed(['FAIL x']))
    expect(off.started).toEqual([])
    expect(off.notes).toEqual([])

    const on = harness()
    await on.checkFix.onChecked(personTurn, passed)
    await on.checkFix.onChecked(personTurn, { command: 'npm test', outcome: 'timed-out', newLines: [], unchanged: false, first: false })
    await on.checkFix.onChecked(personTurn, { command: 'npm test', outcome: 'could-not-run', newLines: ['not found'], unchanged: false, first: false })
    expect(on.started).toEqual([])
    expect(on.notes).toEqual([])
  })

  it('never sends back a turn the person did not send, a comparison column, a reply between teammates or a stopped turn', async () => {
    const h = harness({ sentByPerson: false })
    await h.checkFix.onChecked(personTurn, failed(['FAIL x']))
    const on = harness()
    await on.checkFix.onChecked({ ...personTurn, apart: true }, failed(['FAIL x']))
    await on.checkFix.onChecked({ ...personTurn, completed: false }, failed(['FAIL x']))
    await on.checkFix.onChecked({ ...personTurn, startedBy: { kind: 'routine' } }, failed(['FAIL x']))
    await on.checkFix.onChecked({ ...personTurn, startedBy: { kind: 'carry-on' } }, failed(['FAIL x']))
    expect([...h.started, ...on.started]).toEqual([])
    expect([...h.notes, ...on.notes]).toEqual([])
  })

  it('says nothing about a failure the turn did not cause: the card under it already says nothing in it is new', async () => {
    const h = harness()
    await h.checkFix.onChecked(personTurn, failed(['FAIL old'], ['FAIL old']))
    expect(h.started).toEqual([])
    expect(h.notes).toEqual([])
  })

  it('a fix that cleared some of it is sent back again with what is left, never with what was failing before', async () => {
    const h = harness({ most: 3 })
    await h.checkFix.onChecked(personTurn, failed(['FAIL old', 'FAIL a', 'FAIL b'], ['FAIL old']))
    // Nothing in the second check is new -- yet the fix moved: `FAIL a` is gone.
    await h.checkFix.onChecked(fixTurn('fix_1', 1), failed(['FAIL old', 'FAIL b'], ['FAIL old', 'FAIL a', 'FAIL b']))

    expect(h.started).toHaveLength(2)
    const second = h.started[1]
    expect(second?.followUpOf).toBe('fix_1')
    expect(second?.startedBy).toEqual({ kind: 'check-fix', fix: 2, of: 3, command: 'npm test' })
    expect(second?.prompt).toContain('The check `npm test` still fails after your fix (fix 2 of 3). What it printed that was not failing before your change:')
    expect(second?.prompt).toContain('    FAIL b')
    expect(second?.prompt).not.toContain('FAIL a')
    expect(second?.prompt).not.toContain('FAIL old')
    expect(h.notes).toEqual([])
  })

  it('stops when a fix leaves the failure exactly as it was', async () => {
    const h = harness({ most: 3 })
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    await h.checkFix.onChecked(fixTurn('fix_1', 1), failed(['FAIL a'], ['FAIL a']))
    expect(h.started).toHaveLength(1)
    expect(h.notes).toEqual([{ missionId: 'fix_1', message: CHECK_FIX_STOPPED.noProgress(1), level: 'warning' }])
  })

  it('stops, and says it is done, when all that still fails was failing before the person\'s turn', async () => {
    const h = harness({ most: 3 })
    await h.checkFix.onChecked(personTurn, failed(['FAIL old', 'FAIL a'], ['FAIL old']))
    await h.checkFix.onChecked(fixTurn('fix_1', 1), failed(['FAIL old'], ['FAIL old', 'FAIL a']))
    expect(h.started).toHaveLength(1)
    expect(h.notes).toEqual([{ missionId: 'fix_1', message: CHECK_FIX_STOPPED.fixedWhatItBroke(1), level: 'info' }])
  })

  it('stops at the most, and says how to go on', async () => {
    const h = harness({ most: 1 })
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    await h.checkFix.onChecked(fixTurn('fix_1', 1), failed(['FAIL c'], ['FAIL a']))
    expect(h.started).toHaveLength(1)
    expect(h.notes).toEqual([{ missionId: 'fix_1', message: CHECK_FIX_STOPPED.outOfFixes(1), level: 'warning' }])
    expect(CHECK_FIX_STOPPED.outOfFixes(1)).toContain('after 1 fix,')
    expect(CHECK_FIX_STOPPED.outOfFixes(2)).toContain('after 2 fixes,')
  })

  it('a pass ends it, with nothing more said', async () => {
    const h = harness({ most: 3 })
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    await h.checkFix.onChecked(fixTurn('fix_1', 1), passed)
    expect(h.started).toHaveLength(1)
    expect(h.notes).toEqual([])
  })

  it('a fix that changed no files is said, since its check did not run again', async () => {
    const h = harness()
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    await h.checkFix.onChecked(fixTurn('fix_1', 1), 'no-edits')
    expect(h.started).toHaveLength(1)
    expect(h.notes).toEqual([{ missionId: 'fix_1', message: CHECK_FIX_STOPPED.changedNothing(1), level: 'warning' }])
  })

  it('leaves the conversation alone when the person got there first, and says so', async () => {
    const h = harness({ busy: true })
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    expect(h.started).toEqual([])
    expect(h.notes).toEqual([{ missionId: 'turn_1', message: CHECK_FIX_STOPPED.busy('Wren'), level: 'info' }])
  })

  it('a start refused because the person got there first is said as theirs, not as a failure (0.813)', async () => {
    const h = harness({ start: async () => ({ ok: false, message: 'This conversation went on before the fix could start.', busy: true }) })
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    expect(h.notes).toEqual([{ missionId: 'turn_1', message: CHECK_FIX_STOPPED.busy('Wren'), level: 'info' }])
  })

  it('a second word about the same check starts one fix, not two', async () => {
    const h = harness()
    const check = failed(['FAIL a'], [])
    await Promise.all([h.checkFix.onChecked(personTurn, check), h.checkFix.onChecked(personTurn, check)])
    expect(h.started).toHaveLength(1)
  })

  it('a fix that could not start says why, and the same check may be tried again', async () => {
    let refuse = true
    const h = harness({ start: async () => (refuse ? { ok: false, message: 'Wren is signed out.' } : { ok: true, missionId: 'fix_1' }) })
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    expect(h.notes).toEqual([{ missionId: 'turn_1', message: CHECK_FIX_STOPPED.couldNotStart('Wren is signed out.'), level: 'warning' }])
    refuse = false
    await h.checkFix.onChecked(personTurn, failed(['FAIL a'], []))
    expect(h.started).toHaveLength(2)
  })

  it('after a restart, a fix with no chain behind it is judged against the check before it', async () => {
    const h = harness({ most: 3 })
    await h.checkFix.onChecked(fixTurn('fix_9', 1), failed(['FAIL a'], ['FAIL a']))
    expect(h.notes).toEqual([{ missionId: 'fix_9', message: CHECK_FIX_STOPPED.noProgress(1), level: 'warning' }])
    await h.checkFix.onChecked(fixTurn('fix_8', 1), failed(['FAIL z'], ['FAIL a']))
    expect(h.started.map((asked) => asked.startedBy.fix)).toEqual([2])
  })

  it('tells a fix at most the last 30 lines of the failure, each bounded', async () => {
    const h = harness()
    const lines = Array.from({ length: 45 }, (_, index) => `FAIL test ${String(index)} ${'x'.repeat(index === 44 ? 400 : 0)}`.trim())
    await h.checkFix.onChecked(personTurn, failed(lines, []))
    const told = h.started[0]?.prompt.split('\n').filter((line) => line.startsWith('    FAIL')) ?? []
    expect(told).toHaveLength(30)
    expect(told[0]).toBe('    FAIL test 15')
    expect(told.at(-1)?.length).toBeLessThanOrEqual(4 + 300)
    expect(told.at(-1)?.endsWith('…')).toBe(true)
  })
})

describe('the fix prompt and the setting', () => {
  it('a routine’s fix opens with the words the thread reads it by (missionView.ts fixOfTurn)', () => {
    expect(goalFixPrompt({ command: 'node check.js', passed: false, tail: ['x'] }, 1, 2).startsWith('The check `node check.js` failed after the work above (fix 1 of 2). The end of its output:')).toBe(true)
  })

  it('says when every line is listed because there was no earlier check, and when it printed nothing', () => {
    const first = checkFixPrompt({ command: 'npm test', lines: [], first: true }, 1, 2)
    expect(first).toContain('The check `npm test` failed after your last turn (fix 1 of 2). What it printed:')
    expect(first).toContain('    (it printed nothing)')
    expect(checkFixPrompt({ command: 'npm test', lines: ['FAIL a'], first: true }, 2, 2)).toContain('still fails after your fix (fix 2 of 2). What it printed:')
  })

  it('keeps only a whole number of fixes from 1 to 5, for each folder', () => {
    expect([0, 1, 2, 5, 6, 2.5, '2', null, undefined].map(parsedCheckFixTries)).toEqual([undefined, 1, 2, 5, undefined, undefined, undefined, undefined, undefined])
    expect(parsedCheckFixes({ ws_a: 2, ws_b: 9, ws_c: 'x', '': 3 })).toEqual({ ws_a: 2 })
    expect(parsedCheckFixes({ ws_b: 9 })).toBeUndefined()
    expect(parsedCheckFixes([2])).toBeUndefined()
    expect(parsedCheckFixes('ws_a')).toBeUndefined()
  })
})
