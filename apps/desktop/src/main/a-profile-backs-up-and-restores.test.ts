import { mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { readFileSync, readdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, it } from 'vitest'

import { AWAY_FILE } from './away.js'
import { BACKGROUND_FILE } from './keep-running.js'
import {
  BACKED_UP_FILES,
  BACKED_UP_FOLDERS,
  BACKUP_MANIFEST,
  LAST_RESTORE_FILE,
  LEFT_OUT,
  PENDING_RESTORE_FILE,
  applyPendingRestore,
  backupFolderName,
  compareVersions,
  readBackup,
  requestRestore,
  takeLastRestore,
  writeBackup
} from './profile-backup.js'

/**
 * A PROFILE BACKS UP AND RESTORES (0.614; the PRD's R21).
 *
 * "There is no backup or restore and no export of a profile. A new laptop or a
 * wiped profile loses what the setup sprint built." A folder with a manifest,
 * keys excluded by design, restore with a preview from the same reader, a
 * newer backup refused and a live run refused. The restore is applied at the
 * next start, before any store is read, and what it replaces is moved aside.
 */
const folders: string[] = []
afterEach(async () => {
  for (const folder of folders.splice(0)) await rm(folder, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 })
})
const temp = async (prefix: string): Promise<string> => {
  const folder = await mkdtemp(join(tmpdir(), prefix))
  folders.push(folder)
  return folder
}
const AT = new Date(2026, 9, 4, 20, 15, 0)
const json = (value: unknown): string => `${JSON.stringify(value)}\n`

/** A profile as the app leaves it: the person's stores, the machine's files, a key, the ledger. */
async function profile(teammates: readonly string[] = ['Wren', 'Atlas']): Promise<string> {
  const root = await temp('locust-backup-profile-')
  await writeFile(join(root, 'teammates.json'), json({ schemaVersion: 15, teammates: teammates.map((name) => ({ name })), missionOwners: {}, settings: {} }))
  await writeFile(join(root, 'routines.json'), json({ schemaVersion: 3, routines: [{ name: 'Morning brief' }] }))
  await writeFile(join(root, 'memories.json'), json({ schemaVersion: 2, memories: [{ text: 'a' }, { text: 'b' }, { text: 'c' }], forgotten: [] }))
  await writeFile(join(root, 'approval-rules.json'), json({ schemaVersion: 1, rules: [{ id: 'r1' }] }))
  await writeFile(join(root, 'groups.json'), json({ schemaVersion: 1, groups: [], members: {} }))
  await writeFile(join(root, 'rooms.json'), json({ schemaVersion: 1, rooms: [{ id: 'room' }] }))
  await writeFile(join(root, 'compares.json'), json({ schemaVersion: 1, compares: [] }))
  await writeFile(join(root, 'folders.json'), json({ schemaVersion: 1, folders: [{ path: 'C:/work' }] }))
  // The machine's and the account's: never carried.
  await writeFile(join(root, 'own-models.json'), json({ keys: { openai: 'sk-live-THIS-MUST-NEVER-LEAVE' } }))
  await writeFile(join(root, 'claude-cloud.json'), json({ sessions: ['s1'] }))
  await writeFile(join(root, 'window.json'), json({ width: 1440 }))
  await mkdir(join(root, 'mission-ledger', '.trash'), { recursive: true })
  await writeFile(join(root, 'mission-ledger', 'mission_a.jsonl'), '{"recordType":"mission.created"}\n')
  await writeFile(join(root, 'mission-ledger', 'mission_b.jsonl'), '{"recordType":"mission.created"}\n')
  await writeFile(join(root, 'mission-ledger', '.trash', 'mission_c.jsonl'), '{"recordType":"mission.created"}\n')
  await writeFile(join(root, 'mission-ledger', '.writable'), '')
  await mkdir(join(root, 'workroom'), { recursive: true })
  await writeFile(join(root, 'workroom', 'workroom.jsonl'), '{"message":"hi"}\n')
  return root
}

async function backup(root: string): Promise<string> {
  const parent = await temp('locust-backup-to-')
  const written = await writeBackup(root, parent, { appVersion: '0.614.0', now: () => AT })
  if (!written.ok) throw new Error(written.reason)
  return written.folder
}

describe('a backup', () => {
  it('is a new folder holding the person\'s stores and conversations, each named in its manifest with its hash', async () => {
    const root = await profile()
    const parent = await temp('locust-backup-to-')
    const written = await writeBackup(root, parent, { appVersion: '0.614.0', now: () => AT })
    expect(written.ok).toBe(true)
    if (!written.ok) return
    expect(written.folder).toBe(join(parent, 'Locust backup 2026-10-04 2015'))
    expect(written.counts).toEqual({ teammates: 2, routines: 1, memories: 3, rules: 1, groups: 0, rooms: 1, compares: 0, folders: 1, conversations: 2 })
    const manifest = JSON.parse(await readFile(join(written.folder, BACKUP_MANIFEST), 'utf8')) as { files: { path: string; bytes: number }[]; leftOut: string[]; appVersion: string; schema: number }
    const paths = manifest.files.map((file) => file.path).sort()
    expect(paths).toEqual([
      'approval-rules.json', 'compares.json', 'folders.json', 'groups.json', 'memories.json',
      'mission-ledger/.trash/mission_c.jsonl', 'mission-ledger/mission_a.jsonl', 'mission-ledger/mission_b.jsonl',
      'rooms.json', 'routines.json', 'teammates.json', 'workroom/workroom.jsonl'
    ])
    expect(manifest.appVersion).toBe('0.614.0')
    expect(manifest.schema).toBe(1)
    expect(manifest.leftOut).toContain('own-models.json')
  })

  it('never carries a key, a sign-in or the machine\'s own files: not in the manifest, not in the folder', async () => {
    const root = await profile()
    const folder = await backup(root)
    const everything: string[] = []
    const walk = async (dir: string): Promise<void> => {
      for (const entry of await readdir(dir, { withFileTypes: true })) {
        if (entry.isDirectory()) await walk(join(dir, entry.name))
        else everything.push(await readFile(join(dir, entry.name), 'utf8'))
      }
    }
    await walk(folder)
    expect(everything.join('\n')).not.toContain('sk-live-THIS-MUST-NEVER-LEAVE')
    for (const name of ['own-models.json', 'claude-cloud.json', 'window.json', 'mission-ledger/.writable']) {
      expect(await stat(join(folder, 'profile', ...name.split('/'))).catch(() => undefined), name).toBeUndefined()
    }
  })

  it('is never written into a folder already there: the same minute twice is refused, not overwritten', async () => {
    const root = await profile()
    const parent = await temp('locust-backup-to-')
    expect((await writeBackup(root, parent, { appVersion: '0.614.0', now: () => AT })).ok).toBe(true)
    const again = await writeBackup(root, parent, { appVersion: '0.614.0', now: () => AT })
    expect(again.ok).toBe(false)
  })
})

describe('the reader, for the preview and the restore alike', () => {
  it('reads a whole backup', async () => {
    const reading = await readBackup(await backup(await profile()), { appVersion: '0.614.0' })
    expect(reading.ok && reading.manifest.counts.teammates).toBe(2)
  })

  it('refuses a folder with no manifest: not a backup, or one that did not finish', async () => {
    const reading = await readBackup(await temp('locust-not-a-backup-'), { appVersion: '0.614.0' })
    expect(reading.ok).toBe(false)
    expect(!reading.ok && reading.reason).toMatch(/no locust-backup\.json/)
  })

  it('refuses a backup with a file changed since it was made', async () => {
    const folder = await backup(await profile())
    await writeFile(join(folder, 'profile', 'teammates.json'), json({ schemaVersion: 15, teammates: [{ name: 'Mallory' }] }))
    const reading = await readBackup(folder, { appVersion: '0.614.0' })
    expect(!reading.ok && reading.reason).toMatch(/teammates\.json has changed/)
  })

  it('refuses a backup from a newer Locust, and a newer schema, and says to update', async () => {
    const folder = await backup(await profile())
    expect(await readBackup(folder, { appVersion: '0.613.0' })).toMatchObject({ ok: false, reason: expect.stringMatching(/newer than this one.*Update Locust/) })
    const manifest = JSON.parse(await readFile(join(folder, BACKUP_MANIFEST), 'utf8')) as Record<string, unknown>
    await writeFile(join(folder, BACKUP_MANIFEST), json({ ...manifest, schema: 2 }))
    expect(await readBackup(folder, { appVersion: '0.614.0' })).toMatchObject({ ok: false, reason: expect.stringMatching(/newer Locust/) })
  })

  it('refuses a manifest that names a file a backup never carries, or one outside the profile', async () => {
    for (const path of ['own-models.json', '../teammates.json', 'mission-ledger/../../x']) {
      const folder = await backup(await profile())
      const manifest = JSON.parse(await readFile(join(folder, BACKUP_MANIFEST), 'utf8')) as { files: { path: string }[] }
      manifest.files[0] = { ...manifest.files[0]!, path }
      await writeFile(join(folder, BACKUP_MANIFEST), json(manifest))
      expect(await readBackup(folder, { appVersion: '0.614.0' }), path).toMatchObject({ ok: false, reason: expect.stringMatching(/should not carry/) })
    }
  })

  it('orders versions as numbers, part by part', () => {
    expect(compareVersions('0.614.0', '0.99.0')).toBe(1)
    expect(compareVersions('0.614.0', '0.614.0')).toBe(0)
    expect(compareVersions('0.613.9', '0.614.0')).toBe(-1)
    expect(backupFolderName(AT)).toBe('Locust backup 2026-10-04 2015')
  })
})

describe('a restore', () => {
  it('is refused while a run is going, and nothing is written down', async () => {
    const root = await profile()
    const asked = await requestRestore(await backup(root), root, { appVersion: '0.614.0', liveRuns: 1, now: () => AT })
    expect(asked).toMatchObject({ ok: false, reason: expect.stringMatching(/A run is going/) })
    expect(await stat(join(root, PENDING_RESTORE_FILE)).catch(() => undefined)).toBeUndefined()
  })

  it('is written down when asked, and applied at the next start: the backup in, what it replaced moved aside, the keys untouched', async () => {
    const source = await profile(['Wren', 'Atlas'])
    const folder = await backup(source)
    const here = await profile(['Juno'])
    await writeFile(join(here, 'mission-ledger', 'mission_z.jsonl'), '{"recordType":"mission.created"}\n')
    expect((await requestRestore(folder, here, { appVersion: '0.614.0', liveRuns: 0, now: () => AT })).ok).toBe(true)
    const outcome = await applyPendingRestore(here, { appVersion: '0.614.0', now: () => AT })
    expect(outcome).toMatchObject({ ok: true, counts: { teammates: 2, conversations: 2 } })
    expect(JSON.parse(await readFile(join(here, 'teammates.json'), 'utf8')).teammates).toEqual([{ name: 'Wren' }, { name: 'Atlas' }])
    // The conversation that was here and not in the backup is aside, not lost.
    expect(readdirSync(join(here, 'mission-ledger')).sort()).toEqual(['.trash', 'mission_a.jsonl', 'mission_b.jsonl'])
    const aside = outcome?.aside ?? ''
    expect(JSON.parse(await readFile(join(aside, 'teammates.json'), 'utf8')).teammates).toEqual([{ name: 'Juno' }])
    expect(readdirSync(join(aside, 'mission-ledger'))).toContain('mission_z.jsonl')
    // The key and the machine's files were never touched.
    expect(await readFile(join(here, 'own-models.json'), 'utf8')).toContain('sk-live-THIS-MUST-NEVER-LEAVE')
    expect(await stat(join(here, 'window.json')).catch(() => undefined)).toBeDefined()
    // Asked once, done once; and said once.
    expect(await stat(join(here, PENDING_RESTORE_FILE)).catch(() => undefined)).toBeUndefined()
    expect(await applyPendingRestore(here, { appVersion: '0.614.0', now: () => AT })).toBeUndefined()
    expect(await takeLastRestore(here)).toMatchObject({ ok: true })
    expect(await takeLastRestore(here)).toBeUndefined()
  })

  it('read again at the start: a backup changed since it was asked for is refused, and nothing here moves', async () => {
    const folder = await backup(await profile(['Wren']))
    const here = await profile(['Juno'])
    await requestRestore(folder, here, { appVersion: '0.614.0', liveRuns: 0, now: () => AT })
    await writeFile(join(folder, 'profile', 'routines.json'), json({ schemaVersion: 3, routines: [] }))
    const outcome = await applyPendingRestore(here, { appVersion: '0.614.0', now: () => AT })
    expect(outcome).toMatchObject({ ok: false, reason: expect.stringMatching(/routines\.json has changed/) })
    expect(JSON.parse(await readFile(join(here, 'teammates.json'), 'utf8')).teammates).toEqual([{ name: 'Juno' }])
    expect(readdirSync(here).some((name) => name.startsWith('before-restore-'))).toBe(false)
    expect(await stat(join(here, LAST_RESTORE_FILE)).catch(() => undefined)).toBeDefined()
  })

  it('that fails part-way puts everything back as it was', async () => {
    const folder = await backup(await profile(['Wren', 'Atlas']))
    const here = await profile(['Juno'])
    const before = await readFile(join(here, 'teammates.json'), 'utf8')
    await requestRestore(folder, here, { appVersion: '0.614.0', liveRuns: 0, now: () => AT })
    let copies = 0
    const outcome = await applyPendingRestore(here, {
      appVersion: '0.614.0',
      now: () => AT,
      copy: async (from, to) => {
        copies += 1
        if (copies === 4) throw new Error('the disk is full')
        await writeFile(to, await readFile(from))
      }
    })
    expect(outcome).toMatchObject({ ok: false, reason: expect.stringMatching(/nothing was changed: the disk is full/) })
    expect(await readFile(join(here, 'teammates.json'), 'utf8')).toBe(before)
    expect(readdirSync(join(here, 'mission-ledger')).sort()).toEqual(['.trash', '.writable', 'mission_a.jsonl', 'mission_b.jsonl'])
    expect(readdirSync(here).some((name) => name.startsWith('before-restore-'))).toBe(false)
  })
})

describe('every file the app keeps in its profile is classified', () => {
  /*
   * Named, never "everything but": a store added later is carried or left out
   * by a decision. Every file name the main process joins onto the profile, or
   * onto a store's root, must be in one of the three lists.
   */
  it('backed up, or left out with a reason', () => {
    const main = fileURLToPath(new URL('./', import.meta.url))
    const named = new Set<string>()
    for (const file of readdirSync(main).filter((name) => name.endsWith('.ts') && !name.includes('.test.'))) {
      const source = readFileSync(join(main, file), 'utf8')
      for (const match of source.matchAll(/join\((?:app\.getPath\('userData'\)|options\.rootDirectory|rootDirectory|userData), '([^']+)'\)/g)) named.add(match[1]!)
      // And every store named through a constant, wherever it is joined: the literal-only scan let eight stores drop
      // out of every backup unsaid (the first Mac run's move-in check, 2026-10-30).
      for (const match of source.matchAll(/export const [A-Z_]+_FILE = '([^']+\.json)'/g)) named.add(match[1]!)
    }
    named.add(AWAY_FILE)
    named.add(BACKGROUND_FILE)
    expect(named.size).toBeGreaterThan(20)
    const classified = new Set<string>([...BACKED_UP_FILES, ...BACKED_UP_FOLDERS, ...Object.keys(LEFT_OUT)])
    expect([...named].filter((name) => !classified.has(name))).toEqual([])
  })
})
