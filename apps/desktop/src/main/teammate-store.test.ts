import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { seedAvatar } from '../shared/avatar.js'
import { createTeammateStore, MAX_MISSION_OWNERS, parsedTeammate, validName } from './teammate-store.js'

const roots: string[] = []

async function store() {
  const root = await mkdtemp(join(tmpdir(), 'locust-teammates-'))
  roots.push(root)
  return { root, store: createTeammateStore({ rootDirectory: root }) }
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

describe('a teammate as a bot', () => {
  it('keeps the bot the person picked, and only the fields it knows', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({
      name: 'Sable',
      hue: 'pearl',
      role: 'Code & Migrations',
      avatar: { ...seedAvatar('draft'), bot: { shape: 'swarm', face: 'mouth', glow: true }, extra: 'x' }
    })
    expect(made.hue).toBe('pearl')
    expect(made.avatar.bot).toEqual({ shape: 'swarm', face: 'mouth' })
    expect(Object.keys(made.avatar).sort()).toEqual(['accessory', 'bot', 'headwear', 'mouth'])
  })

  it('refuses a shape there is no bot for', async () => {
    const { store: teammates } = await store()
    await expect(
      teammates.create({ name: 'Nope', hue: 'lime', role: 'Code & Migrations', avatar: { ...seedAvatar('draft'), bot: { shape: 'robot', face: 'eyes' } } })
    ).rejects.toThrow(/avatar is invalid/)
  })

  it('takes every one of the nine colours', async () => {
    const { store: teammates } = await store()
    for (const hue of ['lime', 'blue', 'violet', 'clay', 'teal', 'butter', 'rose', 'slate', 'pearl']) {
      await expect(teammates.create({ name: `Mate ${hue}`, hue, role: 'Code & Migrations' })).resolves.toMatchObject({ hue })
    }
  })
})

const NUL = String.fromCharCode(0)

describe('teammate names', () => {
  it('accepts an ordinary name and trims it', async () => {
    const { store: teammates } = await store()
    const created = await teammates.create({ name: '  Wren  ', hue: 'lime', role: 'Code & Migrations' })
    expect(created.name).toBe('Wren')
  })

  it('refuses control characters rather than silently rewriting them', () => {
    // A name is shown back to the user; quietly stripping what they typed is
    // worse than refusing it.
    expect(validName(`Wren${NUL}`)).toBe(false)
    expect(validName('Wren' + String.fromCharCode(27) + '[31m')).toBe(false)
    expect(validName('Wren')).toBe(true)
  })

  it('refuses empty, whitespace-only and over-long names', () => {
    expect(validName('')).toBe(false)
    expect(validName('   ')).toBe(false)
    expect(validName('x'.repeat(41))).toBe(false)
    expect(validName('x'.repeat(40))).toBe(true)
  })
})

describe('teammate store', () => {
  it('persists a teammate and reads it back from disk', async () => {
    const { root, store: teammates } = await store()
    const created = await teammates.create({ name: 'Atlas', hue: 'blue', role: 'Research & Briefs' })

    const reopened = createTeammateStore({ rootDirectory: root })
    const list = await reopened.list()

    expect(list).toHaveLength(1)
    expect(list[0]).toMatchObject({ name: 'Atlas', hue: 'blue', role: 'Research & Briefs' })
    expect(created.teammateId).toMatch(/^tm_[0-9a-f]{24}$/)
  })

  it('rejects an unknown hue or role instead of storing it', async () => {
    const { store: teammates } = await store()
    await expect(teammates.create({ name: 'X', hue: 'chartreuse', role: 'Code & Migrations' })).rejects.toThrow()
    await expect(teammates.create({ name: 'X', hue: 'lime', role: 'Overlord' })).rejects.toThrow()
    expect(await teammates.list()).toEqual([])
  })

  it('drops a malformed record but keeps the rest of the roster usable', async () => {
    const { root, store: teammates } = await store()
    const good = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    const path = join(root, 'teammates.json')
    const file = JSON.parse(await readFile(path, 'utf8'))
    file.teammates.push({ teammateId: 'tm_bad', name: '', hue: 'lime', role: 'Code & Migrations', createdAt: 'x' })
    await writeFile(path, JSON.stringify(file), 'utf8')

    const list = await createTeammateStore({ rootDirectory: root }).list()

    // The file is in the user's profile and anything can edit it. One bad row
    // must not take the roster down with it.
    expect(list.map((entry) => entry.teammateId)).toEqual([good.teammateId])
  })

  it('says so when it drops a record, rather than dropping it in silence', async () => {
    const { root, store: teammates } = await store()
    await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    const path = join(root, 'teammates.json')
    const file = JSON.parse(await readFile(path, 'utf8'))
    // A role this store does not accept. Exactly what a hand-edited roster,
    // or a test harness seeding one, gets wrong first.
    file.teammates.push({ teammateId: 'tm_gem', name: 'Gem', hue: 'clay', role: 'Scout', createdAt: '2026-09-05T05:00:00.000Z' })
    await writeFile(path, JSON.stringify(file), 'utf8')

    const said: string[] = []
    const warn = console.warn
    console.warn = (message: unknown) => said.push(String(message))
    try {
      await createTeammateStore({ rootDirectory: root }).list()
    } finally {
      console.warn = warn
    }

    // Dropping is right; dropping WITHOUT SAYING cost an hour of chasing a
    // threading defect that did not exist (2026-09-06).
    expect(said.some((line) => /dropped 1 teammate record/.test(line))).toBe(true)
  })

  it('says nothing when every record parses', async () => {
    const { root, store: teammates } = await store()
    await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })

    const said: string[] = []
    const warn = console.warn
    console.warn = (message: unknown) => said.push(String(message))
    try {
      await createTeammateStore({ rootDirectory: root }).list()
    } finally {
      console.warn = warn
    }

    // The negative control: a warning that always fires says nothing at all.
    expect(said).toEqual([])
  })

  it('treats a file from an unknown schema as unreadable rather than guessing -- or overwriting', async () => {
    const { root } = await store()
    const foreign = JSON.stringify({ schemaVersion: 99, teammates: [{ teammateId: 'tm_x' }] })
    await writeFile(join(root, 'teammates.json'), foreign, 'utf8')
    const reopened = createTeammateStore({ rootDirectory: root })
    await expect(reopened.list()).rejects.toThrow('TEAMMATES_UNREADABLE')
    await expect(reopened.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })).rejects.toThrow('TEAMMATES_UNREADABLE')
    expect(await readFile(join(root, 'teammates.json'), 'utf8')).toBe(foreign)
  })

  it('treats a corrupt file as unreadable, and refuses to write over it', async () => {
    /*
     * This used to read as an empty roster. Every write reads first, and
     * `assignMission` writes at every mission start, so a file that could
     * not be read for a moment -- a lock, a scan, a torn byte -- would have
     * been replaced by an empty one on the next start. Found sweeping for
     * the class on 2026-09-16.
     */
    const { root } = await store()
    await writeFile(join(root, 'teammates.json'), 'not json at all', 'utf8')
    const reopened = createTeammateStore({ rootDirectory: root })
    await expect(reopened.list()).rejects.toThrow('TEAMMATES_UNREADABLE')
    await expect(reopened.assignMission('tm_x', 'mission_1')).rejects.toThrow('TEAMMATES_UNREADABLE')
    expect(await readFile(join(root, 'teammates.json'), 'utf8')).toBe('not json at all')
  })

  it('assigns missions and forgets them when the teammate is removed', async () => {
    const { store: teammates } = await store()
    const wren = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    await teammates.assignMission(wren.teammateId, 'mission_1')
    expect(await teammates.missionOwners()).toEqual({ mission_1: wren.teammateId })

    await teammates.remove(wren.teammateId)

    // A mission pointing at a teammate who no longer exists reads as
    // unassigned, never as a dangling reference the UI has to guess about.
    expect(await teammates.list()).toEqual([])
    expect(await teammates.missionOwners()).toEqual({})
  })

  it('refuses to assign a mission to a teammate that does not exist', async () => {
    const { store: teammates } = await store()
    await expect(teammates.assignMission('tm_ghost', 'mission_1')).rejects.toThrow()
  })

  it('refuses unsafe ids', async () => {
    const { store: teammates } = await store()
    const wren = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Docs & QA' })
    await expect(teammates.assignMission(wren.teammateId, '../escape')).rejects.toThrow()
    await expect(teammates.remove('../../etc')).rejects.toThrow()
  })

  it('serializes concurrent creates without losing any', async () => {
    const { store: teammates } = await store()
    await Promise.all([
      teammates.create({ name: 'A', hue: 'lime', role: 'Docs & QA' }),
      teammates.create({ name: 'B', hue: 'blue', role: 'Docs & QA' }),
      teammates.create({ name: 'C', hue: 'violet', role: 'Docs & QA' })
    ])
    expect((await teammates.list()).map((entry) => entry.name).sort()).toEqual(['A', 'B', 'C'])
  })

  it('leaves no temporary files behind', async () => {
    const { root, store: teammates } = await store()
    await teammates.create({ name: 'Wren', hue: 'lime', role: 'Docs & QA' })
    const { readdir } = await import('node:fs/promises')
    expect((await readdir(root)).filter((name) => name.endsWith('.tmp'))).toEqual([])
  })
})

describe('how many missions the roster will hold', () => {
  it('refuses a new assignment past its cap, instead of growing until the file empties itself', async () => {
    // The file has a size cliff: past its byte limit it READS as empty, and
    // the next write saves that empty file over the real one. Unbounded
    // assignments walk straight into it, taking the roster with them.
    const { root, store: teammates } = await store()
    const wren = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })

    const owners: Record<string, string> = {}
    for (let index = 0; index < MAX_MISSION_OWNERS; index += 1) {
      owners[`mission_${String(index)}`] = wren.teammateId
    }
    await writeFile(
      join(root, 'teammates.json'),
      JSON.stringify({ schemaVersion: 1, teammates: [wren], missionOwners: owners, settings: { relay: true } }),
      'utf8'
    )

    const reopened = createTeammateStore({ rootDirectory: root })
    await expect(reopened.assignMission(wren.teammateId, 'mission_one_too_many')).rejects.toThrow()
    // Re-assigning one it already holds is not growth, and stays allowed.
    await expect(reopened.assignMission(wren.teammateId, 'mission_0')).resolves.toBeUndefined()
  })

  it('reads no more assignments than it would write', async () => {
    const { root, store: teammates } = await store()
    const wren = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    const owners: Record<string, string> = {}
    for (let index = 0; index < MAX_MISSION_OWNERS + 50; index += 1) {
      owners[`mission_${String(index)}`] = wren.teammateId
    }
    await writeFile(
      join(root, 'teammates.json'),
      JSON.stringify({ schemaVersion: 1, teammates: [wren], missionOwners: owners, settings: { relay: true } }),
      'utf8'
    )

    const held = await createTeammateStore({ rootDirectory: root }).missionOwners()
    expect(Object.keys(held)).toHaveLength(MAX_MISSION_OWNERS)
  })
})

describe('teammate record parsing', () => {
  it('accepts a well-formed record', () => {
    expect(
      parsedTeammate({
        teammateId: 'tm_1',
        name: 'Wren',
        hue: 'lime',
        role: 'Code & Migrations',
        createdAt: '2026-08-31T16:00:00.000Z'
      })
    ).toBeDefined()
  })

  it('refuses records missing or malforming any field', () => {
    const base = {
      teammateId: 'tm_1',
      name: 'Wren',
      hue: 'lime',
      role: 'Code & Migrations',
      createdAt: '2026-08-31T16:00:00.000Z'
    }
    expect(parsedTeammate({ ...base, teammateId: '../x' })).toBeUndefined()
    expect(parsedTeammate({ ...base, hue: 'gold' })).toBeUndefined()
    expect(parsedTeammate({ ...base, role: 'Anything' })).toBeUndefined()
    expect(parsedTeammate({ ...base, createdAt: 'not a date' })).toBeUndefined()
    expect(parsedTeammate(null)).toBeUndefined()
  })
})

describe("a teammate's own route", () => {
  it('is remembered when a person starts them, survives a rename, and is ignored when malformed', async () => {
    const { root, store: teammates } = await store()
    const booty = await teammates.create({ name: 'Booty', hue: 'violet', role: 'Custom' })
    expect(booty.route).toBeUndefined()

    await teammates.rememberRoute(booty.teammateId, { runtime: 'claude', model: 'sonnet', mode: 'ask' })
    const reopened = createTeammateStore({ rootDirectory: root })
    expect((await reopened.list())[0]?.route).toEqual({ runtime: 'claude', model: 'sonnet', mode: 'ask' })

    // Renaming is theirs to do; it must not cost them their route.
    const renamed = await reopened.update({ ...booty, name: 'Boots' })
    expect(renamed.route).toEqual({ runtime: 'claude', model: 'sonnet', mode: 'ask' })

    // A malformed route, an unknown runtime, or an unknown teammate changes nothing.
    await reopened.rememberRoute(booty.teammateId, { runtime: 'grok-desktop', model: 'x', mode: 'ask' })
    await reopened.rememberRoute(booty.teammateId, 'cursor')
    await reopened.rememberRoute('tm_nobody', { runtime: 'cursor', model: 'composer-2.5', mode: 'accept-edits' })
    expect((await reopened.list())[0]?.route).toEqual({ runtime: 'claude', model: 'sonnet', mode: 'ask' })
  })
})

describe("a Custom teammate's own words for their role", () => {
  it('keeps a title on a Custom role, trimmed and bounded, and drops one on a built-in role', async () => {
    const { root, store: teammates } = await store()
    const custom = await teammates.create({ name: 'Sable', hue: 'clay', role: 'Custom', roleTitle: '  Release manager  ' })
    expect(custom.roleTitle).toBe('Release manager')
    const builtIn = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Docs & QA', roleTitle: 'ignored' })
    expect(builtIn.roleTitle).toBeUndefined()
    // Survives a reopen, and an edit that keeps the role.
    const reopened = createTeammateStore({ rootDirectory: root })
    expect((await reopened.list()).find((entry) => entry.name === 'Sable')?.roleTitle).toBe('Release manager')
    const edited = await reopened.update({ ...custom, name: 'Sable', roleTitle: 'Reviews every PR for security' })
    expect(edited.roleTitle).toBe('Reviews every PR for security')
  })

  it('a title that is too long, empty, or not text is dropped rather than rejected', async () => {
    const { store: teammates } = await store()
    for (const bad of ['x'.repeat(61), '   ', 42, { title: 'x' }, 'line' + String.fromCharCode(10) + 'break']) {
      const made = await teammates.create({ name: 'T', hue: 'lime', role: 'Custom', roleTitle: bad })
      expect(made.roleTitle).toBeUndefined()
      await teammates.remove(made.teammateId)
    }
  })
})

describe('workspace settings', () => {
  it('keeps the autonomy budget inside its bounds, and reads garbage as the default', async () => {
    // 0.21.2 QA, rec. 6: the hop cap becomes the person's own number. It is
    // a workspace-wide bound on spend, so a malformed value must not widen it.
    const { root, store: teammates } = await store()
    expect((await teammates.readSettings()).relayHopCap).toBe(12)
    await teammates.writeSettings({ relay: true, relayHopCap: 2 })
    expect((await teammates.readSettings()).relayHopCap).toBe(2)
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).relayHopCap).toBe(2)
    // 25 is past the bound now, and the old bound of 13 is inside it: the
    // budget counts the WHOLE exchange rather than one chain of it, and six
    // was firing as the ordinary ending rather than as a backstop.
    for (const value of [0, 25, 2.5, '4', -1, null, undefined, Number.NaN]) {
      await teammates.writeSettings({ relay: true, relayHopCap: value })
      expect((await teammates.readSettings()).relayHopCap, String(value)).toBe(12)
    }
    await teammates.writeSettings({ relay: true, relayHopCap: 24 })
    expect((await teammates.readSettings()).relayHopCap).toBe(24)
  })


  it('has teammate replies on by default, and only a literal false turns them off', async () => {
    // Talking to each other is the point of having teammates; the hop cap
    // bounds the spend. So absent or malformed keeps them on, and only an
    // explicit false -- the person's own switch -- turns them off.
    const { root, store: teammates } = await store()
    expect((await teammates.readSettings()).relay).toBe(true)
    for (const value of ['false', 0, null, undefined]) {
      await teammates.writeSettings({ relay: value })
      expect((await teammates.readSettings()).relay).toBe(true)
    }
    await teammates.writeSettings({ relay: false })
    expect(await createTeammateStore({ rootDirectory: root }).readSettings()).toEqual({ relay: false, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: true, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true, plush: false, accountOnChip: false, claudeOwnSkills: false })
  })

  // L19 (the code review): a write replaced the whole settings object, so a
  // caller that left fields out reset them -- choosing Auto reset the layout
  // and every switch reset the send button's metal.
  it('keeps what a write leaves out', async () => {
    const { store: teammates } = await store()
    await teammates.writeSettings({ layout: 'wide', tube: 'subtle', metal: 'gold', metalStrength: 'strong' })
    await teammates.writeSettings({ autoMode: true })
    expect(await teammates.readSettings()).toMatchObject({ autoMode: true, layout: 'wide', tube: 'subtle', metal: 'gold', metalStrength: 'strong' })
    // And a malformed field is still read as the old rules read it.
    await teammates.writeSettings({ autoMode: 'yes' })
    expect((await teammates.readSettings()).autoMode).toBe(false)
  })

  it('reads the defaults, and keeps no swarm: a write naming it, or an old file holding it, sets nothing (0.880)', async () => {
    const { root, store: teammates } = await store()
    expect(await teammates.readSettings()).toEqual({ relay: true, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: true, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true, plush: false, accountOnChip: false, claudeOwnSkills: false })

    await teammates.writeSettings({ swarm: true } as never)

    expect(await createTeammateStore({ rootDirectory: root }).readSettings()).toEqual({ relay: true, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: true, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true, plush: false, accountOnChip: false, claudeOwnSkills: false })
  })

  it('only a literal true turns Auto on', async () => {
    // A malformed message must not be able to enable a workspace-wide setting.
    const { store: teammates } = await store()
    for (const value of ['true', 1, {}, [], null, undefined]) {
      await teammates.writeSettings({ autoMode: value as never, relay: false })
      expect(await teammates.readSettings()).toEqual({ relay: false, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: true, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true, plush: false, accountOnChip: false, claudeOwnSkills: false })
    }
  })

  it('reads a corrupt settings block as the defaults without losing the roster', async () => {
    const { root, store: teammates } = await store()
    const wren = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Docs & QA' })
    const path = join(root, 'teammates.json')
    const file = JSON.parse(await readFile(path, 'utf8'))
    file.settings = 'not an object'
    await writeFile(path, JSON.stringify(file), 'utf8')

    const reopened = createTeammateStore({ rootDirectory: root })
    expect(await reopened.readSettings()).toEqual({ relay: true, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: true, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true, plush: false, accountOnChip: false, claudeOwnSkills: false })
    expect((await reopened.list()).map((entry) => entry.teammateId)).toEqual([wren.teammateId])
  })

  it('keeps teammates when settings are written', async () => {
    const { store: teammates } = await store()
    await teammates.create({ name: 'Wren', hue: 'lime', role: 'Docs & QA' })
    await teammates.writeSettings({ relay: false })
    expect(await teammates.list()).toHaveLength(1)
  })
})

describe('teammate faces', () => {
  it('seeds a face from the id, never the name', async () => {
    const { store: teammates } = await store()
    const first = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    // Two teammates may not share a name at once (A2.18), so the same name
    // comes back as a new teammate: removed, then made again.
    await teammates.remove(first.teammateId)
    const second = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    expect(first.avatar).toEqual(seedAvatar(first.teammateId))
    expect(second.avatar).toEqual(seedAvatar(second.teammateId))
    // Same name, different ids: a face is identity, and identity is the id.
    expect(first.teammateId).not.toBe(second.teammateId)
  })

  it('keeps the look chosen in the dialog, and refuses one outside the tables', async () => {
    const { root, store: teammates } = await store()
    const chosen = { headwear: 4 as const, accessory: 1 as const, mouth: 3 as const }
    const created = await teammates.create({ name: 'Atlas', hue: 'blue', role: 'Research & Briefs', avatar: chosen })
    expect(created.avatar).toEqual(chosen)
    const reopened = await createTeammateStore({ rootDirectory: root }).list()
    expect(reopened[0]?.avatar).toEqual(chosen)
    await expect(
      teammates.create({ name: 'Nova', hue: 'violet', role: 'Docs & QA', avatar: { headwear: 9, accessory: 0, mouth: 0 } })
    ).rejects.toThrow('avatar is invalid')
  })

  it('keeps a persisted face rather than re-seeding it, and seeds one for a record without', () => {
    const base = { teammateId: 'tm_abc', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-01T00:00:00.000Z' }
    const chosen = { headwear: 2, accessory: 2, mouth: 1 }
    expect(parsedTeammate({ ...base, avatar: chosen })?.avatar).toEqual(chosen)
    // A roster written before faces were persisted: the id's face, the same
    // one every reader derives, so nothing changes on upgrade.
    expect(parsedTeammate(base)?.avatar).toEqual(seedAvatar('tm_abc'))
    expect(parsedTeammate({ ...base, avatar: { headwear: 'cap' } })?.avatar).toEqual(seedAvatar('tm_abc'))
  })
})

describe('editing a teammate', () => {
  it('changes name, hue, role and face while the id and its missions stay', async () => {
    const { root, store: teammates } = await store()
    const created = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    await teammates.assignMission(created.teammateId, 'mission_1')
    const look = { headwear: 5 as const, accessory: 2 as const, mouth: 2 as const }

    const updated = await teammates.update({
      teammateId: created.teammateId,
      name: '  Wrenna ',
      hue: 'violet',
      role: 'Docs & QA',
      avatar: look
    })

    expect(updated).toEqual({ ...created, name: 'Wrenna', hue: 'violet', role: 'Docs & QA', avatar: look })
    const reopened = createTeammateStore({ rootDirectory: root })
    expect(await reopened.list()).toEqual([updated])
    expect(await reopened.missionOwners()).toEqual({ mission_1: created.teammateId })
  })

  it('refuses an unknown teammate and an invalid field, changing nothing', async () => {
    const { store: teammates } = await store()
    const created = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    await expect(
      teammates.update({ teammateId: 'tm_nobody', name: 'X', hue: 'lime', role: 'Custom', avatar: created.avatar })
    ).rejects.toThrow('Unknown teammate')
    await expect(
      teammates.update({ teammateId: created.teammateId, name: 'X', hue: 'lime', role: 'Custom', avatar: { headwear: 99 } })
    ).rejects.toThrow('avatar is invalid')
    expect(await teammates.list()).toEqual([created])
  })
})

describe('memory mode', () => {
  it('defaults to keeping memory and telling, keeps ask or off, and reads anything else as the default', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-teammates-'))
    roots.push(root)
    const teammates = createTeammateStore({ rootDirectory: root })
    expect((await teammates.readSettings()).memoryMode).toBe('auto')
    expect((await teammates.writeSettings({ relay: true, relayHopCap: 12, memoryMode: 'ask' })).memoryMode).toBe('ask')
    expect((await teammates.writeSettings({ relay: true, relayHopCap: 12, memoryMode: 'off' })).memoryMode).toBe('off')
    expect((await teammates.writeSettings({ relay: true, relayHopCap: 12, memoryMode: 'sometimes' } as never)).memoryMode).toBe('auto')
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).memoryMode).toBe('auto')
  })
})

/**
 * A teammate's own folder.
 *
 * Colin, 2026-09-09: "it should only change the folder for that chat/teammate
 * not the entire app." The project folder switch reopens Locust because the
 * ledger, memory and worktrees are all scoped by it; this is the narrower
 * thing, and it has to survive every other edit made to the record.
 */
describe("a teammate's own folder", () => {
  it('is kept, cleared, and never left behind as an empty key', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    expect(made.folder).toBeUndefined()

    // A rooted POSIX path, which node calls absolute on Windows too, so one
    // fixture reads the same sentence on both platforms.
    const home = '/home/<home>/claude'
    const pointed = await teammates.setFolder(made.teammateId, home)
    expect(pointed.folder).toBe(home)
    expect((await teammates.list())[0]?.folder).toBe(home)

    const cleared = await teammates.setFolder(made.teammateId, undefined)
    // The KEY has to go, not just its value: `{...record, folder: undefined}`
    // writes `"folder": undefined`, which JSON drops on the way out and the
    // next reader cannot tell from a folder that failed to save.
    expect('folder' in cleared).toBe(false)
  })

  it('survives an edit of the name, the face and the branch switch', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    // A rooted POSIX path, which node calls absolute on Windows too, so one
    // fixture reads the same sentence on both platforms.
    const home = '/home/<home>/claude'
    await teammates.setFolder(made.teammateId, home)
    // The renderer never sends a folder, so an update that rebuilt the record
    // from the request alone would silently move the teammate back.
    const edited = await teammates.update({
      teammateId: made.teammateId,
      name: 'Wren the second',
      hue: 'blue',
      role: 'Code & Migrations',
      worktree: true,
      avatar: seedAvatar(made.teammateId)
    })
    expect(edited.name).toBe('Wren the second')
    expect(edited.worktree).toBe(true)
    expect(edited.folder).toBe(home)
  })

  it('refuses anything that is not an absolute path', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    // A relative path resolves against whatever the app's own process happens
    // to be standing in, which is not a folder anybody chose.
    await expect(teammates.setFolder(made.teammateId, 'claude')).rejects.toThrow()
    await expect(teammates.setFolder(made.teammateId, '')).rejects.toThrow()
    await expect(teammates.setFolder(made.teammateId, '   ')).rejects.toThrow()
    await expect(teammates.setFolder('tm_nobody', '/tmp')).rejects.toThrow()
    expect((await teammates.list())[0]?.folder).toBeUndefined()
  })

  it('drops a relative folder read back from a hand-edited file', () => {
    const base = {
      teammateId: 'tm_wren',
      name: 'Wren',
      hue: 'lime',
      role: 'Code & Migrations',
      createdAt: '2026-09-05T05:00:00.000Z'
    }
    expect(parsedTeammate({ ...base, folder: 'claude' })?.folder).toBeUndefined()
    expect(parsedTeammate({ ...base, folder: 12 })?.folder).toBeUndefined()
    // A rooted POSIX path, which node calls absolute on Windows too, so one
    // fixture reads the same sentence on both platforms.
    const home = '/home/<home>/claude'
    expect(parsedTeammate({ ...base, folder: home })?.folder).toBe(home)
  })
})

/**
 * "Ask before every connector call" is a switch a person turns on, and only
 * a person: a file from before the field, a malformed value, or a string
 * that merely looks like true all read as the ordinary state.
 */
describe('asking before every connector call', () => {
  it('is off unless the file says exactly true', async () => {
    const { store: teammates } = await store()
    expect((await teammates.readSettings()).askConnectors).toBe(false)
    for (const wrong of ['true', 1, 'yes', {}] as const) {
      const written = await teammates.writeSettings({ relay: true, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: wrong as never, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true })
      expect(written.askConnectors, String(wrong)).toBe(false)
    }
  })

  it('stays on across a write of some other setting', async () => {
    // Every write carries the whole object; a caller that forgot this field
    // would switch it off as a side effect of changing the layout.
    const { store: teammates } = await store()
    await teammates.writeSettings({ relay: true, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: true, layout: 'auto', tube: 'full', replySize: 'standard', codeFont: 'geist', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true })
    const after = await teammates.writeSettings({ relay: true, relayHopCap: 12, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: true, layout: 'rail' })
    expect(after.askConnectors).toBe(true)
    expect((await teammates.readSettings()).askConnectors).toBe(true)
  })
})


/**
 * A teammate narrowed to some connectors stays narrowed, and an empty list
 * is "everything" -- the key goes, not a present-and-empty list that would
 * read as nothing.
 */
describe("a teammate's own connectors", () => {
  it('narrow, widen back, and never leave an empty key behind', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ name: 'Jimothy', hue: 'lime', role: 'Custom' })
    expect(made.connectors).toBeUndefined()
    const narrowed = await teammates.setConnectors(made.teammateId, ['claude.ai Robinhood', 'claude.ai Robinhood', '  '])
    expect(narrowed.connectors).toEqual(['claude.ai Robinhood'])
    const widened = await teammates.setConnectors(made.teammateId, [])
    expect('connectors' in widened).toBe(false)
  })

  it('survive an edit of the name, and drop anything that is not a plain name', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ name: 'Jimothy', hue: 'lime', role: 'Custom' })
    await teammates.setConnectors(made.teammateId, ['claude.ai Gmail', 12, 'x'.repeat(200), 'ok' + String.fromCharCode(7)] as never)
    const edited = await teammates.update({ teammateId: made.teammateId, name: 'Jim', hue: 'blue', role: 'Custom', avatar: seedAvatar(made.teammateId) })
    expect(edited.connectors).toEqual(['claude.ai Gmail'])
    await expect(teammates.setConnectors('tm_nobody', ['claude.ai Gmail'])).rejects.toThrow()
  })
})

describe('the plan a teammate keeps', () => {
  /*
   * Colin, 2026-09-14: "plans are off by default, it should be on unless we
   * find issues."
   *
   * Flipping the constant was not enough. The parse read the key with
   * `=== true`, so an ABSENT key meant off -- and every settings file that
   * already exists has no such key, which is every person who has ever run
   * this. The default would have changed for nobody.
   *
   * Absent means never chosen, and never chosen means the default.
   */
  it('is on when nobody has said otherwise', async () => {
    const { store: teammates } = await store()
    expect((await teammates.readSettings()).keepATodoList).toBe(true)
  })

  it('is on for a settings file written before the setting existed', async () => {
    const { root } = await store()
    // A file from before this setting existed: no `keepATodoList` at all.
    await writeFile(
      join(root, 'teammates.json'),
      JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { relay: true } }),
      'utf8'
    )
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).keepATodoList).toBe(true)
  })

  it('stays off once somebody turns it off', async () => {
    // The other half: a real choice has to survive, or the default is a
    // setting that cannot be changed.
    const { root, store: teammates } = await store()
    await teammates.writeSettings({ keepATodoList: false })
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).keepATodoList).toBe(false)
  })

  it('reads anything that is not a boolean as the default', async () => {
    const { store: teammates } = await store()
    for (const value of ['false', 0, {}, [], null]) {
      await teammates.writeSettings({ keepATodoList: value })
      expect((await teammates.readSettings()).keepATodoList).toBe(true)
    }
  })
})

/**
 * Colin, 2026-09-21: "have cursor bend automatically off, user can turn on in
 * settings if they like." The flourish on the send button is the kind of
 * thing a new user should meet only by going looking for it.
 *
 * The half that matters is the second test: flipping a default is a no-op for
 * everyone who already has a settings file unless an ABSENT key is what reads
 * as the default. The todo-list setting learned this the hard way.
 */
describe('the cursor bend is off until somebody asks for it', () => {
  it('is off on a fresh profile', async () => {
    const { store: teammates } = await store()
    expect((await teammates.readSettings()).metalBend).toBe(false)
  })

  it('still honours a stored choice, in both directions', async () => {
    const { root, store: teammates } = await store()
    const settings = await teammates.readSettings()
    await teammates.writeSettings({ ...settings, metalBend: true })
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).metalBend).toBe(true)
    await teammates.writeSettings({ ...settings, metalBend: false })
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).metalBend).toBe(false)
  })
})

/**
 * Colin, 2026-10-03: "they might all need a screen for a face, we can have it
 * togglable in settings, terminal face, and if the user chooses to have it off
 * it will revert back to our previous eyes." On for everyone, existing
 * profiles too (an absent key reads as on); a stored off stays off, and a
 * write of some other setting keeps it.
 */
describe('terminal faces are on until somebody turns them off', () => {
  it('is on on a fresh profile', async () => {
    const { store: teammates } = await store()
    expect((await teammates.readSettings()).terminalFaces).toBe(true)
  })

  it('stays off once turned off, across a write of something else', async () => {
    const { root, store: teammates } = await store()
    await teammates.writeSettings({ ...(await teammates.readSettings()), terminalFaces: false })
    await teammates.writeSettings({ relay: false })
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).terminalFaces).toBe(false)
    await teammates.writeSettings({ relay: false, terminalFaces: true })
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).terminalFaces).toBe(true)
  })
})

describe("a teammate's monthly limit", () => {
  const base = { name: 'Wren', hue: 'lime', role: 'Code & Migrations' } as const

  it('is kept when set, and an edit that never mentions money keeps it', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ ...base, monthlyLimitUsd: 5 })
    expect(made.monthlyLimitUsd).toBe(5)
    const renamed = await teammates.update({ ...base, teammateId: made.teammateId, name: 'Wren II', avatar: made.avatar })
    expect(renamed.monthlyLimitUsd).toBe(5)
    expect((await teammates.list())[0]?.monthlyLimitUsd).toBe(5)
  })

  it('is changed by an amount and lifted by null', async () => {
    const { store: teammates } = await store()
    const made = await teammates.create({ ...base, monthlyLimitUsd: 5 })
    expect((await teammates.update({ ...base, teammateId: made.teammateId, avatar: made.avatar, monthlyLimitUsd: 12.5 })).monthlyLimitUsd).toBe(12.5)
    const lifted = await teammates.update({ ...base, teammateId: made.teammateId, avatar: made.avatar, monthlyLimitUsd: null })
    expect(lifted.monthlyLimitUsd).toBeUndefined()
    expect('monthlyLimitUsd' in ((await teammates.list())[0] ?? {})).toBe(false)
  })

  it('refuses an amount that is not one, rather than saving a limit nobody meant', async () => {
    const { store: teammates } = await store()
    await expect(teammates.create({ ...base, monthlyLimitUsd: 0 })).rejects.toThrow(/limit is invalid/)
    await expect(teammates.create({ ...base, monthlyLimitUsd: -5 })).rejects.toThrow(/limit is invalid/)
    await expect(teammates.create({ ...base, monthlyLimitUsd: '5' })).rejects.toThrow(/limit is invalid/)
    const made = await teammates.create(base)
    await expect(teammates.update({ ...base, teammateId: made.teammateId, avatar: made.avatar, monthlyLimitUsd: Number.NaN })).rejects.toThrow(/limit is invalid/)
  })

  it('reads a limit that does not parse as NO limit -- never as zero, which would refuse every run', () => {
    const record = { teammateId: 'tm_x', name: 'X', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-26T00:00:00.000Z', monthlyLimitUsd: 'lots' }
    expect(parsedTeammate(record)?.monthlyLimitUsd).toBeUndefined()
    expect(parsedTeammate({ ...record, monthlyLimitUsd: 7.25 })?.monthlyLimitUsd).toBe(7.25)
  })
})
