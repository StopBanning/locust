/** W7: "Routines that ask for inputs, and travel as files"; assert the exported key set. */
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import type { PublicRoutine, PublicTeammate } from '../shared/ipc.js'
import { seedAvatar } from '../shared/avatar.js'
import type { RoutineInput } from '../shared/routine-inputs.js'
import { inputsRefusal, resolveValues, substituteSteps, validInputs } from '../shared/routine-inputs.js'
import { createRoutineStore, parsedRoutine } from './routine-store.js'
import { absolutePathsIn, parseRoutineFile, pathsAsInputs, routineFileText, routineToFile } from './routine-file.js'
import { createRoutineIO } from './routine-io.js'
import { fillInputs } from './routine-runner.js'

const route = { runtime: 'opencode', model: 'opencode/free-test', mode: 'ask' } as const
const inputs: readonly RoutineInput[] = [
  { key: 'topic', label: 'Topic', kind: 'text', required: true, default: 'default topic' },
  { key: 'tone', label: 'Tone', kind: 'choice', required: true, choices: ['Brief', 'Detailed'], default: 'Brief' }
]
const fresh = { name: 'Notes', steps: ['Say {{topic}} in {{tone}} form.'], inputs, teammateId: 'tm_one', route, learnedFrom: [] }
const routine: PublicRoutine = { ...fresh, routineId: 'rt_one', createdAt: '2026-10-03T00:00:00.000Z', runs: 0 }
const roots: string[] = []
async function root(): Promise<string> { const path = await mkdtemp(join(tmpdir(), 'locust-routine-inputs-test-')); roots.push(path); return path }
afterEach(async () => { await Promise.all(roots.splice(0).map((path) => rm(path, { recursive: true, force: true }))) })
const toFile = (overrides: Partial<PublicRoutine> = {}) => routineToFile({ ...routine, ...overrides }, { steps: routine.steps, inputs, roleOf: () => 'Finance Bro', connectorNames: [] })

describe('a routine asks for bounded declared inputs', () => {
  it('keeps all four kinds and refuses bad keys, duplicates, extra fields, defaults and choices', () => {
    expect(validInputs([...inputs, { key: 'note', label: 'Note', kind: 'long-text', required: false, default: 'two\nlines' }, { key: 'folder', label: 'Folder', kind: 'folder', required: true }])).toBe(true)
    for (const bad of [
      [{ ...inputs[0], key: 'Topic' }], [{ ...inputs[0], key: 'a'.repeat(33) }], [inputs[0], inputs[0]],
      [{ ...inputs[0], extra: 'secret' }], [{ ...inputs[0], default: 'a'.repeat(4001) }],
      [{ ...inputs[0], label: 'two\nlines' }], [{ ...inputs[1], default: 'Other' }],
      [{ ...inputs[1], choices: ['same', 'same'] }], [{ ...inputs[1], choices: ['Only'] }],
      [{ ...inputs[0], kind: 'folder', default: 'C:\\local' }], Array.from({ length: 13 }, (_, at) => ({ ...inputs[0], key: `key_${at}` }))
    ]) expect(inputsRefusal(bad)).toBeTypeOf('string')
    expect(inputsRefusal(inputs, ['Read {{missing}}'])).toContain('not declared')
    expect(inputsRefusal(inputs, ['Read {{Topic}}'])).toContain('not declared')
    expect(inputsRefusal(inputs, ['Read {{}}'])).toContain('not declared')
  })

  it('round-trips declarations in the real store and refuses undeclared create/update markers', async () => {
    const store = createRoutineStore({ rootDirectory: await root() })
    const saved = await store.create(fresh)
    expect((await store.get(saved.routineId))?.inputs).toEqual(inputs)
    await expect(store.create({ ...fresh, inputs: [] })).rejects.toThrow('not declared')
    await expect(store.update({ routineId: saved.routineId, name: 'New', steps: ['{{topic}}'], inputs: null })).rejects.toThrow('not declared')
    const changed = await store.update({ routineId: saved.routineId, name: 'New', steps: ['{{topic}}'], inputs: [inputs[0]] })
    expect(changed.inputs).toEqual([inputs[0]])
    expect(parsedRoutine({ ...changed, inputs: [{ ...inputs[0], key: 'BAD' }] })).toBeUndefined()
    const legacy = parsedRoutine({ ...routine, inputs: undefined, steps: ['Literal {{old}}'] })!
    await writeFile(join(roots.at(-1)!, 'routines.json'), JSON.stringify({ schemaVersion: 1, routines: [legacy] }))
    expect((await store.update({ routineId: legacy.routineId, name: 'Legacy', steps: legacy.steps })).steps).toEqual(legacy.steps)
    await expect(store.update({ routineId: legacy.routineId, name: 'Legacy', steps: legacy.steps, inputs })).rejects.toThrow('not declared')
  })

  it('uses defaults, refuses missing required values and substitutes text only once', () => {
    expect(resolveValues(inputs, undefined)).toEqual({ ok: true, values: { topic: 'default topic', tone: 'Brief' } })
    expect(resolveValues([{ ...inputs[0]!, default: undefined }], undefined)).toMatchObject({ ok: false, message: expect.stringContaining('cannot start on its own') })
    expect(resolveValues([{ ...inputs[0]!, default: undefined }], {})).toMatchObject({ ok: false, message: expect.stringContaining('needs a value') })
    for (const value of [null, [], { stranger: 'x' }, { topic: 42 }, { topic: 'a'.repeat(4001) }, { topic: 'two\nlines' }, { tone: 'Other' }]) expect(resolveValues(inputs, value).ok).toBe(false)
    expect(resolveValues([{ key: 'constructor', label: 'Name', kind: 'text', required: true, default: 'okay' }], undefined)).toEqual({ ok: true, values: { constructor: 'okay' } })
    expect(substituteSteps(['{{topic}} / {{tone}} / {{topic}}'], { topic: '$& $(no shell) {{tone}}', tone: 'Brief' })).toEqual(['$& $(no shell) {{tone}} / Brief / $& $(no shell) {{tone}}'])
    expect(fillInputs({ ...routine, steps: ['{{topic}}'.repeat(4)] }, { topic: 'a'.repeat(4000), tone: 'Brief' })).toMatchObject({ ok: false, message: expect.stringContaining('too long') })
    const folderRoutine = { ...routine, steps: ['Read {{folder}}'], inputs: [{ key: 'folder', label: 'Folder', kind: 'folder', required: true } as const] }
    expect(fillInputs(folderRoutine, { folder: 'C:\\chosen' }).ok).toBe(false)
    expect(fillInputs(folderRoutine, { folder: 'C:\\chosen' }, (path) => path === 'C:\\chosen')).toMatchObject({ ok: true, routine: { steps: ['Read C:\\chosen'] } })
  })
})

describe('a routine file carries only its portable definition', () => {
  it('asserts the exact top-level and nested key sets, with machine and execution data absent', () => {
    const exported = JSON.parse(routineFileText(toFile({ workspaceId: 'ws_private', learnedFrom: ['mission_private'], runs: 9,
      schedule: { kind: 'every', hours: 4 }, handOffs: [{ teammateId: 'tm_private', check: true }] })))
    expect(Object.keys(exported).sort()).toEqual(['format', 'version', 'name', 'steps', 'inputs', 'handOffs', 'route', 'connectors'].sort())
    expect(Object.keys(exported.route)).toEqual(['runtime'])
    // The role and whether the step checks -- both portable; never the teammate (2026-10-05: a chain template carries its checker).
    expect(Object.keys(exported.handOffs[0])).toEqual(['role', 'check'])
    expect(Object.keys(exported.inputs[0]).sort()).toEqual(['key', 'label', 'kind', 'required', 'default'].sort())
    expect(JSON.stringify(exported)).not.toMatch(/tm_private|mission_private|ws_private|schedule|runs|staged|execution|learnedFrom/)
    expect(parseRoutineFile(JSON.stringify(exported))).toEqual({ ok: true, file: exported })
  })

  it('refuses another format/version, hidden fields, invalid nested fields and undeclared placeholders', () => {
    const file = toFile()
    for (const bad of [{ ...file, format: 'other' }, { ...file, version: 2 }, { ...file, schedule: {} },
      { ...file, route: { runtime: 'opencode', model: 'secret' } }, { ...file, handOffs: [{ teammateId: 'tm_one' }] },
      { ...file, inputs: [] }, { ...file, inputs: [{ ...inputs[0], token: 'secret' }] }]) expect(parseRoutineFile(JSON.stringify(bad)).ok).toBe(false)
    expect(parseRoutineFile('not json').ok).toBe(false)
    const { inputs: _inputs, ...missingInputs } = { ...file, steps: ['ordinary step'] }
    expect(parseRoutineFile(JSON.stringify(missingInputs)).ok).toBe(false)
    expect(parseRoutineFile(' '.repeat(512 * 1024 + 1)).ok).toBe(false)
  })

  it('flags Windows and home paths inside prose, and converts repeated paths without changing the source', () => {
    const steps = ['Read C:\\work\\notes, then /home/test/docs.', 'Read C:\\work\\notes again.']
    expect(absolutePathsIn(steps)).toEqual([{ step: 1, path: 'C:\\work\\notes' }, { step: 1, path: '/home/test/docs' }, { step: 2, path: 'C:\\work\\notes' }])
    const changed = pathsAsInputs(steps, [{ key: 'path', label: 'Existing', kind: 'text', required: false }])
    expect(changed).toMatchObject({ ok: true, steps: ['Read {{path_2}}, then {{path_3}}.', 'Read {{path_2}} again.'] })
    expect(absolutePathsIn(['Read "C:\\My Folder\\file.md".'])).toEqual([{ step: 1, path: 'C:\\My Folder\\file.md' }])
    expect(pathsAsInputs(['Read "C:\\My Folder\\file.md".'], [])).toMatchObject({ ok: true, steps: ['Read "{{path}}".'], inputs: [{ key: 'path', kind: 'text', required: true }] })
    expect(steps[0]).toContain('C:\\work')
  })
})

describe('main owns folder choices and import/export receipts', () => {
  async function harness() {
    const directory = await root()
    const store = createRoutineStore({ rootDirectory: directory })
    const saved = await store.create(fresh)
    const file = join(directory, 'Notes.locust-routine.json')
    const team: readonly PublicTeammate[] = [{ teammateId: 'tm_one', name: 'One', role: 'Custom', hue: 'lime', avatar: seedAvatar('tm_one'), route, createdAt: routine.createdAt }]
    const run = vi.fn().mockResolvedValue({ ok: true, data: { missionId: 'mission', runId: 'run' } })
    const picker = vi.fn().mockResolvedValue(file)
    let workspace: string | undefined = 'ws_test'
    const io = createRoutineIO({ routines: store, team: async () => team, connectors: () => ['present'], workspace: () => workspace,
      pickFolder: async () => directory, pickExport: picker, pickImport: picker, run })
    return { directory, store, saved, file, run, picker, io, move: (id: string | undefined) => { workspace = id } }
  }

  it('refuses typed folder paths until the host picker selected them; malformed values start nothing', async () => {
    const h = await harness()
    const saved = await h.store.create({ ...fresh, steps: ['Read {{folder}}'], inputs: [{ key: 'folder', label: 'Folder', kind: 'folder', required: true }] })
    expect((await h.io.run(saved.routineId, { folder: h.directory })).ok).toBe(false)
    expect((await h.io.run(saved.routineId, undefined)).ok).toBe(false)
    expect(h.run).not.toHaveBeenCalled()
    await h.io.folder()
    expect((await h.io.run(saved.routineId, { folder: h.directory })).ok).toBe(true)
    expect(h.run).toHaveBeenCalledWith(saved.routineId, { folder: h.directory })
    h.run.mockClear()
    expect((await h.io.run(h.saved.routineId, null)).ok).toBe(false)
    expect(h.run).not.toHaveBeenCalled()
  })

  it('flags paths before opening a save dialog, then writes only the converted export', async () => {
    const h = await harness()
    const saved = await h.store.create({ ...fresh, steps: ['Read C:\\work\\notes.'], inputs: [] })
    expect(await h.io.export({ routineId: saved.routineId })).toMatchObject({ ok: false, error: { code: 'ABSOLUTE_PATHS' } })
    expect(h.picker).not.toHaveBeenCalled()
    expect((await h.io.export({ routineId: saved.routineId, paths: 'input' })).ok).toBe(true)
    // Written as Markdown a person reads (0.834), named for it, and read back as the routine it is.
    expect(h.picker).toHaveBeenCalledWith('Notes.routine.md')
    const written = await readFile(h.file, 'utf8')
    expect(written).toContain('\n# Notes\n\n## Step 1\n\nRead {{path}}.\n')
    const read = parseRoutineFile(written)
    if (!read.ok) throw new Error(read.message)
    const exported = read.file
    expect(Object.keys(exported).sort()).toEqual(['format', 'version', 'name', 'steps', 'inputs', 'handOffs', 'route', 'connectors'].sort())
    expect(exported.steps).toEqual(['Read {{path}}.'])
    expect((await h.store.get(saved.routineId))?.steps).toEqual(['Read C:\\work\\notes.'])
    expect(h.run).not.toHaveBeenCalled()
  })

  it('previews connector presence without creating or running; imports only once, unscheduled and read-only', async () => {
    const h = await harness()
    await writeFile(h.file, routineFileText({ ...toFile(), connectors: ['present', 'missing'] }))
    const answer = await h.io.preview()
    if (!answer.ok || !answer.data.preview) throw new Error('no preview')
    const preview = answer.data.preview
    expect(preview.connectors).toEqual([{ name: 'present', present: true }, { name: 'missing', present: false }])
    expect(await h.store.list()).toHaveLength(1)
    // Changing the on-disk file after preview cannot change the accepted definition.
    await writeFile(h.file, '{"format":"other"}')
    const request = { token: preview.token, teammateId: 'tm_one', route: { ...route, mode: 'auto' } }
    const answers = await Promise.all([h.io.import(request), h.io.import(request)])
    expect(answers.filter((answer) => answer.ok)).toHaveLength(1)
    const imported = answers.find((answer) => answer.ok)!
    expect(imported).toMatchObject({ ok: true, data: { routine: { steps: routine.steps, inputs, runs: 0, learnedFrom: [], workspaceId: 'ws_test', route: { mode: 'ask' } } } })
    if (imported.ok) expect(imported.data.routine?.schedule).toBeUndefined()
    expect((await h.io.import(request)).ok).toBe(false)
    expect(h.run).not.toHaveBeenCalled()
  })

  it('refuses a preview from a different folder, removed teammates and fabricated tokens', async () => {
    const h = await harness()
    await writeFile(h.file, routineFileText(toFile()))
    const answer = await h.io.preview()
    if (!answer.ok || !answer.data.preview) throw new Error('no preview')
    const token = answer.data.preview.token
    expect((await h.io.import({ token: 'invented', teammateId: 'tm_one', route })).ok).toBe(false)
    expect((await h.io.import({ token, teammateId: 'tm_gone', route })).ok).toBe(false)
    h.move('ws_elsewhere')
    expect((await h.io.import({ token, teammateId: 'tm_one', route })).ok).toBe(false)
    expect(await h.store.list()).toHaveLength(1)
    expect(h.run).not.toHaveBeenCalled()
  })
})
