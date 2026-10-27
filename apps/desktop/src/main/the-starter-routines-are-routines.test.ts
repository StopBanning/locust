import { readFileSync, readdirSync } from 'node:fs'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it, vi } from 'vitest'

import type { PublicTeammate } from '../shared/ipc.js'
import { seedAvatar } from '../shared/avatar.js'
import { absolutePathsIn, parseRoutineFile } from './routine-file.js'
import { createRoutineIO } from './routine-io.js'
import { createRoutineStore } from './routine-store.js'
import { TEMPLATE_ORDER, TEMPLATE_SUFFIX, listRoutineTemplates, readRoutineTemplate } from './routine-templates.js'

/**
 * THE STARTER ROUTINES ARE ROUTINES (0.615; the PRD's R16).
 *
 * Shipped as ordinary routine files and read by the reader an import uses, so
 * each must be exactly what a file from a colleague would be -- and written so
 * the import's Ask is all it needs: no route, no path, no connector.
 */
const SHIPPED = fileURLToPath(new URL('../../resources/routines/', import.meta.url))
const files = readdirSync(SHIPPED).filter((name) => name.endsWith(TEMPLATE_SUFFIX))

describe('every starter routine', () => {
  it('is there: sixteen -- the ten of R16, the challenge of its section 7, three chains, the overnight polish and Council review -- each in the order offered', () => {
    expect(files.length).toBe(16)
    expect(files.map((name) => name.slice(0, -TEMPLATE_SUFFIX.length)).sort()).toEqual([...TEMPLATE_ORDER].sort())
  })

  it('reads as a routine file, names no route, no path and no connector, and asks only for what its steps use', () => {
    for (const name of files) {
      const read = parseRoutineFile(readFileSync(join(SHIPPED, name), 'utf8'))
      expect(read.ok, name).toBe(true)
      if (!read.ok) continue
      expect(read.file.route, name).toEqual({})
      expect(read.file.connectors, name).toEqual([])
      expect(absolutePathsIn(read.file.steps), name).toEqual([])
      for (const input of read.file.inputs) expect(read.file.steps.some((step) => step.includes(`{{${input.key}}}`)), `${name}: ${input.key}`).toBe(true)
      // Written for Ask: none tells the teammate to change anything -- but a chain's fix and build steps do, and open in a copy (below).
      if (read.file.handOffs.some((entry) => entry.check === true)) continue
      for (const step of read.file.steps) expect(/\b(edit|write to|modify|delete|commit|push)\b(?! any file)/i.test(step.replace(/do not change (any|the) file/gi, '')), `${name}: ${step.slice(0, 60)}`).toBe(false)
    }
  })

  it('has a name of its own: no two the same', () => {
    const names = files.map((name) => (parseRoutineFile(readFileSync(join(SHIPPED, name), 'utf8')) as { file: { name: string } }).file.name)
    expect(new Set(names).size).toBe(names.length)
  })

  it('ships: the build lays down each one by name', () => {
    const builder = readFileSync(fileURLToPath(new URL('../../electron-builder.yml', import.meta.url)), 'utf8')
    for (const name of files) expect(builder, name).toContain(`      - ${name}`)
  })
})

describe('the list and the reader', () => {
  it('lists them in the order offered, each with what it does and what it asks', async () => {
    const listed = await listRoutineTemplates(SHIPPED)
    expect(listed.refused).toEqual([])
    expect(listed.templates.map((template) => template.id)).toEqual([...TEMPLATE_ORDER])
    const challenge = listed.templates.find((template) => template.id === 'challenge-an-idea')
    expect(challenge).toMatchObject({ name: 'Challenge an idea before building it', steps: 4, asks: ['The idea'] })
    expect(listed.templates.every((template) => template.summary.length > 10)).toBe(true)
  })

  it('reads one by its id, and never a path or a file it does not ship', async () => {
    expect((await readRoutineTemplate(SHIPPED, 'review-a-file')).ok).toBe(true)
    for (const id of ['../routines/review-a-file', '..\\x', 'C:/Windows/win.ini', 'not-a-template', 42]) {
      expect((await readRoutineTemplate(SHIPPED, id)).ok, String(id)).toBe(false)
    }
  })

  it('leaves out, and names, a file in the folder that is not a routine', async () => {
    const folder = await mkdtemp(join(tmpdir(), 'locust-templates-'))
    try {
      await writeFile(join(folder, `broken${TEMPLATE_SUFFIX}`), '{"format":"locust-routine"}')
      await writeFile(join(folder, `good${TEMPLATE_SUFFIX}`), readFileSync(join(SHIPPED, files[0]!), 'utf8'))
      const listed = await listRoutineTemplates(folder)
      expect(listed.templates.map((template) => template.id)).toEqual(['good'])
      expect(listed.refused[0]).toMatch(/^broken\.locust-routine\.json: /)
    } finally {
      await rm(folder, { recursive: true, force: true })
    }
  })
})

describe('a template is added the way a routine file is imported', () => {
  async function harness(workspace: string | undefined) {
    const folder = await mkdtemp(join(tmpdir(), 'locust-templates-io-'))
    const store = createRoutineStore({ rootDirectory: folder })
    const route = { runtime: 'opencode', model: 'opencode/free-test', mode: 'auto' } as const
    const team: readonly PublicTeammate[] = [{ teammateId: 'tm_one', name: 'One', role: 'Custom', hue: 'lime', avatar: seedAvatar('tm_one'), route, createdAt: '2026-10-04T00:00:00.000Z' }]
    const run = vi.fn()
    const io = createRoutineIO({ routines: store, team: async () => team, connectors: () => [], workspace: () => workspace, templates: SHIPPED,
      pickFolder: async () => undefined, pickImport: async () => undefined, pickExport: async () => undefined, run })
    return { folder, store, io, run, route }
  }

  it('previews by id, then adds it once: Ask, no schedule, in this folder, run by nobody', async () => {
    const h = await harness('ws_test')
    try {
      expect((await h.io.templates()).ok).toBe(true)
      const answer = await h.io.previewTemplate('draft-a-reply')
      if (!answer.ok || answer.data.preview === undefined) throw new Error('no preview')
      expect(answer.data.preview).toMatchObject({ name: 'Draft a reply', connectors: [] })
      expect(await h.store.list()).toHaveLength(0)
      const request = { token: answer.data.preview.token, teammateId: 'tm_one', route: h.route }
      const added = await h.io.import(request)
      expect(added).toMatchObject({ ok: true, data: { routine: { name: 'Draft a reply', workspaceId: 'ws_test', route: { mode: 'ask' }, runs: 0 } } })
      if (added.ok) expect(added.data.routine?.schedule).toBeUndefined()
      expect(await h.io.import(request)).toMatchObject({ ok: false })
      expect(h.run).not.toHaveBeenCalled()
    } finally {
      await rm(h.folder, { recursive: true, force: true })
    }
  })

  it('asks for a folder first in its own words, not the words for a file', async () => {
    const h = await harness(undefined)
    try {
      const answer = await h.io.previewTemplate('explain-this-project')
      if (!answer.ok || answer.data.preview === undefined) throw new Error('no preview')
      const refused = await h.io.import({ token: answer.data.preview.token, teammateId: 'tm_one', route: h.route })
      expect(refused).toMatchObject({ ok: false, error: { message: expect.stringContaining('Choose the folder your teammates work in first') } })
      expect(JSON.stringify(refused)).not.toMatch(/file/i)
      expect(await h.store.list()).toHaveLength(0)
    } finally {
      await rm(h.folder, { recursive: true, force: true })
    }
  })
})
