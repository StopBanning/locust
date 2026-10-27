import { readFileSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { proposedHandOffs, proposeTeammates } from '../shared/chain-proposal.js'
import type { ChainStep } from '../shared/chain-proposal.js'
import type { PublicRoutine, PublicTeammate, TeammateRole } from '../shared/ipc.js'
import { ROLE_DESCRIPTIONS } from '../shared/ipc.js'
import { seedAvatar } from '../shared/avatar.js'
import { absolutePathsIn, parseRoutineFile, routineFileText, routineToFile } from './routine-file.js'
import { createRoutineIO } from './routine-io.js'
import { createRoutineStore } from './routine-store.js'
import { listRoutineTemplates, TEMPLATE_SUFFIX, templateChangesFiles, templateOnAnotherAgent } from './routine-templates.js'

/**
 * READY-MADE CHAINS (2026-10-05): fix a bug, build a feature, make it faster.
 *
 * Each step names a ROLE, not a teammate -- a template cannot know anyone's
 * roster -- and the last step is the checker. Locust proposes a teammate per
 * role from the roster; the editor shows the proposal and the person changes it.
 */
const SHIPPED = fileURLToPath(new URL('../../resources/routines/', import.meta.url))
const CHAINS = ['fix-a-bug', 'build-a-feature', 'make-it-faster'] as const
/** The overnight polish (2026-10-12): a chain too, that asks nothing -- the project's own tests are its question. */
const NIGHTLY = ['polish-overnight'] as const
/** Council review (0.837): three reviewers, each proposed on another agent where the roster has one. */
const COUNCIL = ['council-review'] as const
const read = (id: string) => {
  const parsed = parseRoutineFile(readFileSync(join(SHIPPED, `${id}${TEMPLATE_SUFFIX}`), 'utf8'))
  if (!parsed.ok) throw new Error(`${id}: ${parsed.message}`)
  return parsed.file
}

const mate = (teammateId: string, role: TeammateRole, roleTitle?: string): PublicTeammate =>
  ({ teammateId, name: teammateId.replace('tm_', ''), hue: 'lime', avatar: seedAvatar(teammateId), role, ...(roleTitle === undefined ? {} : { roleTitle }), createdAt: '2026-10-05T00:00:00.000Z', route: { runtime: 'opencode', model: 'opencode/free', mode: 'ask' } }) as PublicTeammate

const FIX: readonly ChainStep[] = [{ role: 'Research & Briefs' }, { role: 'Code & Migrations' }, { role: 'Docs & QA', check: true }]

describe('the chain templates are files', () => {
  it('are three, each a routine of three steps that asks one question its steps use', () => {
    for (const id of CHAINS) {
      const file = read(id)
      expect(file.steps, id).toHaveLength(3)
      expect(file.inputs, id).toHaveLength(1)
      expect(file.steps.some((step) => step.includes(`{{${file.inputs[0]!.key}}}`)), id).toBe(true)
      expect(file.route, id).toEqual({})
      expect(file.connectors, id).toEqual([])
      expect(absolutePathsIn(file.steps), id).toEqual([])
    }
  })

  it('give every step a role the roster can have, and make the last step -- and only the last -- the checker', () => {
    const roles = Object.keys(ROLE_DESCRIPTIONS)
    for (const id of CHAINS) {
      const file = read(id)
      file.handOffs.forEach((entry, at) => {
        expect(roles, `${id} step ${String(at + 1)}`).toContain(entry.role)
        expect(entry.check === true, `${id} step ${String(at + 1)}`).toBe(at === file.handOffs.length - 1)
      })
    }
  })

  it('are written short and plain: a few sentences a step, no persona, nothing that says who the reader is', () => {
    for (const id of CHAINS) {
      for (const step of read(id).steps) {
        expect(step.length, `${id}: ${step.slice(0, 40)}`).toBeLessThan(700)
        expect(/\byou are (a|an|the)\b/i.test(step), `${id}: ${step.slice(0, 40)}`).toBe(false)
      }
    }
  })

  it('say what the check is for in the last step: where it broke, the plan, the same way as before', () => {
    expect(read('fix-a-bug').steps[2]).toMatch(/same surface/)
    expect(read('fix-a-bug').steps[2]).toMatch(/did not or could not test/)
    expect(read('build-a-feature').steps[2]).toMatch(/against the plan/)
    expect(read('make-it-faster').steps[2]).toMatch(/exactly the way it was measured before/)
    expect(read('make-it-faster').steps[2]).toMatch(/before and after/)
  })

  it('are listed as chains, with what they do; the others are not', async () => {
    const listed = await listRoutineTemplates(SHIPPED)
    for (const template of listed.templates) expect(template.chain === true, template.id).toBe(([...CHAINS, ...NIGHTLY, ...COUNCIL] as readonly string[]).includes(template.id))
    expect(listed.templates.find((template) => template.id === 'fix-a-bug')).toMatchObject({ steps: 3, asks: ['What is going wrong?'] })
    expect(listed.templates.every((template) => template.summary.length > 10)).toBe(true)
  })

  it('are the ones that change files; the rest are read-only as ever', () => {
    for (const id of [...CHAINS, ...NIGHTLY]) expect(templateChangesFiles(id), id).toBe(true)
    expect(templateChangesFiles('review-a-file')).toBe(false)
    expect(templateChangesFiles('challenge-an-idea')).toBe(false)
  })
})

describe('the overnight polish', () => {
  it('runs the tests, fixes what fails, and has its fixes checked and reported -- asking nothing, naming no route', () => {
    const file = read('polish-overnight')
    expect(file.steps).toHaveLength(3)
    expect(file.inputs).toEqual([])
    expect(file.route).toEqual({})
    expect(file.connectors).toEqual([])
    expect(file.steps[0]).toMatch(/Do not change any file/)
    expect(file.steps[1]).toMatch(/smallest change/)
    expect(file.steps[2]).toMatch(/morning report/)
    expect(file.handOffs.map((entry) => entry.check === true)).toEqual([false, false, true])
  })
})

describe('the file format carries the checker', () => {
  it('reads a check marker, and refuses a check that is not true or a field it does not know', () => {
    const text = (handOffs: unknown) => JSON.stringify({ format: 'locust-routine', version: 1, name: 'n', steps: ['a'], inputs: [], handOffs, route: {}, connectors: [] })
    expect(parseRoutineFile(text([{ role: 'Docs & QA', check: true }]))).toMatchObject({ ok: true, file: { handOffs: [{ role: 'Docs & QA', check: true }] } })
    expect(parseRoutineFile(text([{ check: false }])).ok).toBe(false)
    expect(parseRoutineFile(text([{ check: 'yes' }])).ok).toBe(false)
    expect(parseRoutineFile(text([{ teammateId: 'tm_x' }])).ok).toBe(false)
  })

  it('writes a chain with every step\'s role and its checker, so another Locust can propose for each', () => {
    const routine = { name: 'Chain', teammateId: 'tm_wren', steps: ['a', 'b', 'c'], handOffs: [{}, { teammateId: 'tm_atlas' }, { teammateId: 'tm_sable', check: true }], route: { runtime: 'opencode' } } as unknown as PublicRoutine
    const roles: Record<string, string> = { tm_wren: 'Code & Migrations', tm_atlas: 'Research & Briefs', tm_sable: 'Docs & QA' }
    const file = routineToFile(routine, { steps: routine.steps, inputs: [], roleOf: (id) => roles[id], connectorNames: [] })
    expect(file.handOffs).toEqual([{ role: 'Code & Migrations' }, { role: 'Research & Briefs' }, { role: 'Docs & QA', check: true }])
    expect(parseRoutineFile(routineFileText(file))).toMatchObject({ ok: true, file: { handOffs: file.handOffs } })
  })

  it('writes a routine with no checker exactly as it always did: roles only for the steps handed on', () => {
    const routine = { name: 'Plain', teammateId: 'tm_wren', steps: ['a', 'b'], handOffs: [{}, { teammateId: 'tm_atlas' }], route: { runtime: 'opencode' } } as unknown as PublicRoutine
    const roles: Record<string, string> = { tm_wren: 'Code & Migrations', tm_atlas: 'Research & Briefs' }
    expect(routineToFile(routine, { steps: routine.steps, inputs: [], roleOf: (id) => roles[id], connectorNames: [] }).handOffs).toEqual([{}, { role: 'Research & Briefs' }])
  })
})

describe('a teammate is proposed for each role', () => {
  it('gives every step to a teammate who is alone on the roster, the last still the checker', () => {
    const wren = mate('tm_wren', 'Custom', 'Everything')
    expect(proposeTeammates(FIX, [wren])).toEqual(['tm_wren', 'tm_wren', 'tm_wren'])
    expect(proposedHandOffs(FIX, [wren])).toEqual({ owner: 'tm_wren', handOffs: [{}, {}, { check: true }] })
  })

  it('goes by role on a full roster: the cause to Research, the fix to Code, the check to Docs & QA', () => {
    const team = [mate('tm_wren', 'Code & Migrations'), mate('tm_atlas', 'Research & Briefs'), mate('tm_sable', 'Docs & QA'), mate('tm_ops', 'Ops & Scheduling')]
    expect(proposeTeammates(FIX, team)).toEqual(['tm_atlas', 'tm_wren', 'tm_sable'])
    // The first step's teammate runs the routine; the others are named where they differ; the checker is marked.
    expect(proposedHandOffs(FIX, team)).toEqual({ owner: 'tm_atlas', handOffs: [{}, { teammateId: 'tm_wren' }, { teammateId: 'tm_sable', check: true }] })
  })

  it('takes the first teammate in roster order who has the role, and reads a custom role by its title', () => {
    const team = [mate('tm_one', 'Code & Migrations'), mate('tm_two', 'Code & Migrations'), mate('tm_qa', 'Custom', 'docs & qa')]
    expect(proposeTeammates(FIX, team)).toEqual(['tm_one', 'tm_one', 'tm_qa'])
  })

  it('does not let whoever did the work check it when there is another teammate to', () => {
    const team = [mate('tm_a', 'Docs & QA'), mate('tm_b', 'Docs & QA')]
    // Research -> a (no Research: continues as the first); Code -> continues; the checker prefers b over a.
    expect(proposeTeammates(FIX, team)).toEqual(['tm_a', 'tm_a', 'tm_b'])
    // With no one else, the one who did it checks it, and the plan says so.
    expect(proposeTeammates(FIX, [mate('tm_a', 'Docs & QA')])).toEqual(['tm_a', 'tm_a', 'tm_a'])
  })

  it('still proposes someone when no role matches: the steps stay with whoever took the one before, the checker goes to another teammate', () => {
    const team = [mate('tm_boss', 'Chief of Staff'), mate('tm_ops', 'Ops & Scheduling')]
    expect(proposeTeammates(FIX, team)).toEqual(['tm_boss', 'tm_boss', 'tm_ops'])
    expect(proposeTeammates(FIX, [mate('tm_boss', 'Chief of Staff')])).toEqual(['tm_boss', 'tm_boss', 'tm_boss'])
  })

  it('keeps a step nobody on the roster is suited to with whoever took the step before it, wherever they are in the roster', () => {
    const team = [mate('tm_boss', 'Chief of Staff'), mate('tm_code', 'Code & Migrations')]
    const steps: readonly ChainStep[] = [{ role: 'Code & Migrations' }, { role: 'Something Nobody Has' }, { role: 'Docs & QA', check: true }]
    expect(proposeTeammates(steps, team)).toEqual(['tm_code', 'tm_code', 'tm_boss'])
  })

  it('proposes nobody for an empty roster, and a step with no role goes where the one before it went', () => {
    expect(proposeTeammates(FIX, [])).toEqual([])
    expect(proposedHandOffs(FIX, [])).toEqual({ owner: undefined, handOffs: [{ }, { }, { check: true }] })
    expect(proposeTeammates([{ role: 'Docs & QA' }, {}, { check: true }], [mate('tm_q', 'Docs & QA'), mate('tm_c', 'Code & Migrations')])).toEqual(['tm_q', 'tm_q', 'tm_c'])
  })

  it('is what the three chains get from the shipped files', () => {
    const team = [mate('tm_wren', 'Code & Migrations'), mate('tm_atlas', 'Research & Briefs'), mate('tm_sable', 'Docs & QA'), mate('tm_data', 'Data & Reporting')]
    const stepsOf = (id: string): readonly ChainStep[] => read(id).handOffs.map((entry) => ({ role: entry.role, check: entry.check === true }))
    expect(proposeTeammates(stepsOf('fix-a-bug'), team)).toEqual(['tm_atlas', 'tm_wren', 'tm_sable'])
    expect(proposeTeammates(stepsOf('build-a-feature'), team)).toEqual(['tm_atlas', 'tm_wren', 'tm_sable'])
    // Measured and re-measured by the same teammate; the one that changes it is another.
    expect(proposeTeammates(stepsOf('make-it-faster'), team)).toEqual(['tm_data', 'tm_wren', 'tm_data'])
  })
})

describe('a chain template, as the window asks for it', () => {
  it('previews with its roles and its checker, and says it changes files; a read-only one says neither', async () => {
    const folder = await mkdtemp(join(tmpdir(), 'locust-chain-'))
    try {
      const io = createRoutineIO({ routines: createRoutineStore({ rootDirectory: folder }), team: async () => [], connectors: () => [], workspace: () => 'ws', templates: SHIPPED,
        pickFolder: async () => undefined, pickImport: async () => undefined, pickExport: async () => undefined, run: async () => ({ ok: false, error: { code: 'ROUTINE_REJECTED', message: 'no' } }) })
      const chain = await io.previewTemplate('fix-a-bug')
      if (!chain.ok || chain.data.preview === undefined) throw new Error('no preview')
      expect(chain.data.preview).toMatchObject({ handOffRoles: ['Research & Briefs', 'Code & Migrations', 'Docs & QA'], handOffChecks: [false, false, true], changesFiles: true })
      const plain = await io.previewTemplate('review-a-file')
      if (!plain.ok || plain.data.preview === undefined) throw new Error('no preview')
      expect(plain.data.preview.handOffChecks).toEqual([false])
      expect(plain.data.preview).not.toHaveProperty('changesFiles')
    } finally {
      await rm(folder, { recursive: true, force: true })
    }
  })
})

describe('Council review: one change read by different AIs (0.837)', () => {
  const on = (teammateId: string, runtime: string): PublicTeammate => ({ ...mate(teammateId, 'Code & Migrations'), route: { runtime, model: 'm', mode: 'ask' } }) as PublicTeammate
  const COUNCIL_STEPS: readonly ChainStep[] = [{}, { another: true }, { check: true, another: true }]

  it('is three read-only steps that ask nothing, the last settling the other two and checking', () => {
    const file = read('council-review')
    expect(file.steps).toHaveLength(3)
    expect(file.inputs).toEqual([])
    expect(file.route).toEqual({})
    for (const step of file.steps) expect(step).toMatch(/Do not change any file\.$/)
    expect(file.steps[1]).toMatch(/agree, or disagree and say why/)
    expect(file.steps[2]).toMatch(/at least two of the three of you/)
    expect(file.handOffs.map((entry) => entry.check === true)).toEqual([false, false, true])
    expect(templateChangesFiles('council-review')).toBe(false)
  })

  it('previews which steps go to another agent; no other template says so', async () => {
    expect(templateOnAnotherAgent('council-review')).toEqual([false, true, true])
    for (const id of [...CHAINS, ...NIGHTLY, 'review-a-file', 'challenge-an-idea']) expect(templateOnAnotherAgent(id), id).toBeUndefined()
    const folder = await mkdtemp(join(tmpdir(), 'locust-council-'))
    try {
      const io = createRoutineIO({ routines: createRoutineStore({ rootDirectory: folder }), team: async () => [], connectors: () => [], workspace: () => 'ws', templates: SHIPPED,
        pickFolder: async () => undefined, pickImport: async () => undefined, pickExport: async () => undefined, run: async () => ({ ok: false, error: { code: 'ROUTINE_REJECTED', message: 'no' } }) })
      const council = await io.previewTemplate('council-review')
      if (!council.ok || council.data.preview === undefined) throw new Error('no preview')
      expect(council.data.preview).toMatchObject({ handOffChecks: [false, false, true], handOffOnAnotherAgent: [false, true, true] })
      expect(council.data.preview).not.toHaveProperty('changesFiles')
      const fix = await io.previewTemplate('fix-a-bug')
      if (!fix.ok || fix.data.preview === undefined) throw new Error('no preview')
      expect(fix.data.preview).not.toHaveProperty('handOffOnAnotherAgent')
    } finally {
      await rm(folder, { recursive: true, force: true })
    }
  })

  it('proposes a teammate on a different agent for each review, where the roster has one', () => {
    const team = [on('tm_claude', 'claude'), on('tm_claude_2', 'claude'), on('tm_codex', 'codex'), on('tm_open', 'opencode')]
    expect(proposeTeammates(COUNCIL_STEPS, team)).toEqual(['tm_claude', 'tm_codex', 'tm_open'])
    expect(proposedHandOffs(COUNCIL_STEPS, team)).toEqual({ owner: 'tm_claude', handOffs: [{}, { teammateId: 'tm_codex' }, { teammateId: 'tm_open', check: true }] })
  })

  it('with two agents, the third review goes to a teammate who has not reviewed yet; with one teammate, all three are theirs', () => {
    expect(proposeTeammates(COUNCIL_STEPS, [on('tm_a', 'claude'), on('tm_b', 'claude'), on('tm_c', 'codex')])).toEqual(['tm_a', 'tm_c', 'tm_b'])
    expect(proposeTeammates(COUNCIL_STEPS, [on('tm_a', 'claude'), on('tm_b', 'claude')])).toEqual(['tm_a', 'tm_b', 'tm_b'])
    expect(proposeTeammates(COUNCIL_STEPS, [on('tm_a', 'claude')])).toEqual(['tm_a', 'tm_a', 'tm_a'])
  })

  it('is what the shipped template gets, read the way the window reads it', () => {
    const file = read('council-review')
    const another = templateOnAnotherAgent('council-review')!
    const steps: readonly ChainStep[] = file.handOffs.map((entry, at) => ({ role: entry.role, check: entry.check === true, another: another[at] }))
    expect(proposeTeammates(steps, [on('tm_claude', 'claude'), on('tm_codex', 'codex'), on('tm_cursor', 'cursor')])).toEqual(['tm_claude', 'tm_codex', 'tm_cursor'])
  })
})
