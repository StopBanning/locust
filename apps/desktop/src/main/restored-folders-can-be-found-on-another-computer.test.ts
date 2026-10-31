import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, expect, it } from 'vitest'
import { createFolderRegistry, folderName } from './folders.js'
import { findRestoredFolders } from './restored-folders.js'
import { workspaceIdFor } from './workspace.js'
import { resumableThreadOf } from './codex-mission.js'
import type { RecoveredMission } from '@teammate/mission-store'
import { isTeammateFolder } from './teammate-store.js'

const roots: string[] = []
afterEach(async () => { for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true }) })
async function setup() {
  const root = await mkdtemp(join(tmpdir(), 'locust-restored-folders-'))
  roots.push(root)
  const local = join(root, 'work', 'demo')
  await mkdir(local, { recursive: true })
  return { root, local, file: join(root, 'folders.json') }
}
it('keeps Windows and Mac paths and their names on either computer', async () => {
  const { file } = await setup()
  const paths = ['C:\\work\\demo', '/Users/old/work/demo']
  await writeFile(file, JSON.stringify({ schemaVersion: 1, folders: paths.map(path => ({ id: workspaceIdFor(path), path })) }))
  expect((await createFolderRegistry({ file }).list()).map(folder => folder.path)).toEqual(paths)
  for (const path of paths) {
    expect(folderName(path)).toBe('demo')
    expect(isTeammateFolder(path)).toBe(true)
  }
})
it('offers a Windows folder on a Mac and a Mac folder on Windows without changing it first', async () => {
  const { file, root, local } = await setup()
  for (const path of ['C:\\work\\demo', '/Users/old/work/demo']) {
    const registry = createFolderRegistry({ file })
    await registry.learn([{ id: workspaceIdFor(path), path }])
    const matches = await findRestoredFolders(await registry.list(), root)
    expect(matches.find(match => match.path === path)?.candidate).toBe(local)
    expect(await registry.pathOf(workspaceIdFor(path))).toBe(path)
  }
})
it('keeps the old conversation id after a chosen move and remembers the move after reopening', async () => {
  const { file, local } = await setup()
  const old = 'C:\\work\\demo'
  const registry = createFolderRegistry({ file })
  await registry.learn([{ id: workspaceIdFor(old), path: old }])
  await registry.move(workspaceIdFor(old), local)
  const reopened = createFolderRegistry({ file })
  expect(await reopened.pathOf(workspaceIdFor(old))).toBe(local)
  expect(await reopened.sameAs(old)).toBe(local)
  expect((await reopened.list())[0]?.previousPath).toBe(old)
})
it('leaves ambiguous folder names for the picker and uses a saved remote to distinguish them', async () => {
  const { root, local } = await setup()
  const other = join(root, 'Projects', 'demo')
  await mkdir(join(other, '.git'), { recursive: true })
  await writeFile(join(other, '.git', 'config'), '[remote "origin"]\n url = git@example.com:team/demo.git\n')
  const folder = { id: 'old', path: 'C:\\work\\demo', name: 'demo' }
  expect((await findRestoredFolders([folder], root))[0]?.candidate).toBeUndefined()
  expect((await findRestoredFolders([{ ...folder, remote: 'https://example.com/team/demo' }], root))[0]?.candidate).toBe(other)
  expect((await findRestoredFolders([{ ...folder, remote: 'https://example.com/another/demo' }], root))[0]?.candidate).toBeUndefined()
  expect((await findRestoredFolders([{ ...folder, path: local }], root))).toEqual([])
})

it('starts a fresh agent session after a recorded folder move', () => {
  const prior = { metadata: { runtime: 'codex', continuesFrom: { runtimeThreadId: 'old-computer-session' } }, events: [{ type: 'adapter.diagnostic', payload: { code: 'host.folder_moved' } }] } as unknown as RecoveredMission
  expect(resumableThreadOf(prior, 'codex')).toBeUndefined()
  expect(resumableThreadOf({ ...prior, events: [] }, 'codex')).toBe('old-computer-session')
})
