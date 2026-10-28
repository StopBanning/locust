import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

/*
 * A TEAMMATE SIGNS IN APART IN THE BROWSER (0.839, the OpenBot scrub's take 2). A teammate with Own browser sign-ins
 * on gets a partition of its own: the person can open a tab as it to sign it in, its tools see and use only its own
 * tabs, no other run's tools reach them, and turning it off clears what it had signed in to. Electron is stood in for:
 * what is checked is which partition each tab is made in and which tabs each run is let near.
 */

const partitions: string[] = []
const cleared: string[] = []
vi.mock('electron', () => {
  const fakeSession = (name: string) => ({
    setSpellCheckerDictionaryDownloadURL: () => undefined,
    setPermissionRequestHandler: () => undefined,
    setPermissionCheckHandler: () => undefined,
    webRequest: { onCompleted: () => undefined, onErrorOccurred: () => undefined },
    on: () => undefined,
    clearStorageData: async () => { cleared.push(name) },
    clearCache: async () => undefined
  })
  let ids = 0
  class WebContentsView {
    readonly partition: string
    readonly webContents = {
      id: (ids += 1),
      isDestroyed: () => false,
      close: () => undefined,
      on: () => undefined,
      setWindowOpenHandler: () => undefined,
      getURL: () => '',
      getTitle: () => '',
      loadURL: async () => undefined,
      navigationHistory: { canGoBack: () => false, canGoForward: () => false }
    }
    constructor(options: { readonly webPreferences: { readonly partition: string } }) {
      this.partition = options.webPreferences.partition
      partitions.push(this.partition)
    }
    setBounds(): void {}
    setVisible(): void {}
  }
  return { BrowserWindow: class {}, WebContentsView, session: { fromPartition: (name: string) => fakeSession(name) } }
})

const { createBrowserHost } = await import('./browser-host.js')
const { createTeammateStore } = await import('./teammate-store.js')
const { seedAvatar } = await import('../shared/avatar.js')

const wren = { id: 'tm_wren', name: 'Wren', hue: 'lime' }
const roots: string[] = []
afterEach(async () => {
  partitions.splice(0)
  cleared.splice(0)
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true })
})

async function hostWith(profiles: Record<string, typeof wren>) {
  const root = await mkdtemp(join(tmpdir(), 'locust-own-browser-'))
  roots.push(root)
  const host = createBrowserHost({
    bridgePath: join(root, 'bridge.mjs'),
    node: process.execPath,
    getWorkspaceRoot: () => root,
    profileOf: async (teammateId) => profiles[teammateId]
  })
  const run = async (runId: string, profile?: typeof wren) => {
    await host.registerRun({ runId, missionId: `mission_${runId}`, teammateLabel: profile?.name ?? 'Sable', cwd: root, ...(profile === undefined ? {} : { profile }) })
    return (args: Record<string, unknown>) => host.callTool(runId, 'browser_tabs', args).then((answer) => answer.content.map((item) => item.text).join('\n'))
  }
  return { host, run }
}

describe('a teammate signs in apart in the browser', () => {
  it('opens a tab as the teammate in a partition of its own, colored as it, beside the shared one', async () => {
    const { host } = await hostWith({ tm_wren: wren })
    await host.newTab()
    const state = await host.newTab(undefined, 'tm_wren')
    expect(partitions).toEqual(['persist:locust-browser', 'persist:locust-browser-tm_wren'])
    expect(state.tabs.map((tab) => tab.profile)).toEqual([undefined, wren])
    await host.dispose()
  })

  it('opens nothing as a teammate that does not have its own sign-ins', async () => {
    const { host } = await hostWith({})
    const state = await host.newTab(undefined, 'tm_wren')
    expect(state.tabs).toEqual([])
    expect(state.error).toBe('That teammate does not have its own browser sign-ins any more, so no tab was opened.')
    await host.dispose()
  })

  it('lets each run see and use only the tabs in its own sign-ins', async () => {
    const { host, run } = await hostWith({ tm_wren: wren })
    await host.newTab()
    await host.newTab(undefined, 'tm_wren')
    const asWren = await run('run_wren', wren)
    const asSable = await run('run_sable')
    expect(await asWren({ action: 'list' })).toBe('Open tabs:\n* tab_2: New Tab (about:blank)')
    expect(await asSable({ action: 'list' })).toBe('Open tabs:\n- tab_1: New Tab (about:blank)')
    // Neither can pick the other's tab by name.
    expect(await asWren({ action: 'select', tab_id: 'tab_1' })).toContain('signed in as someone else, so it is not yours to use')
    expect(await asSable({ action: 'close', tab_id: 'tab_2' })).toContain('signed in as someone else, so it is not yours to use')
    expect(host.getState().tabs).toHaveLength(2)
    await host.dispose()
  })

  it('makes a run its own tab in its own sign-ins when it opens one', async () => {
    const { host, run } = await hostWith({ tm_wren: wren })
    const asWren = await run('run_wren', wren)
    await asWren({ action: 'new' })
    expect(partitions).toEqual(['persist:locust-browser-tm_wren'])
    expect(host.getState().tabs[0]?.profile).toEqual(wren)
    await host.dispose()
  })

  it('forgets them: the teammate’s tabs close and its partition is cleared, the shared tab stays', async () => {
    const { host } = await hostWith({ tm_wren: wren })
    await host.newTab()
    await host.newTab(undefined, 'tm_wren')
    const state = await host.forgetProfile('tm_wren')
    expect(state.tabs.map((tab) => tab.id)).toEqual(['tab_1'])
    expect(cleared).toEqual(['persist:locust-browser-tm_wren'])
    await host.dispose()
  })

  it('is kept on the teammate’s record, only as a literal true, and an edit of its name keeps it', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-own-browser-store-'))
    roots.push(root)
    const teammates = createTeammateStore({ rootDirectory: root })
    const made = await teammates.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    expect(made.ownBrowser).toBeUndefined()
    expect((await teammates.setOwnBrowser(made.teammateId, 'yes')).ownBrowser).toBeUndefined()
    expect((await teammates.setOwnBrowser(made.teammateId, true)).ownBrowser).toBe(true)
    const renamed = await teammates.update({ teammateId: made.teammateId, name: 'Wren 2', hue: 'lime', role: 'Code & Migrations', avatar: seedAvatar(made.teammateId) })
    expect(renamed.ownBrowser).toBe(true)
    expect((await createTeammateStore({ rootDirectory: root }).list()).find((mate) => mate.teammateId === made.teammateId)?.ownBrowser).toBe(true)
    expect((await teammates.setOwnBrowser(made.teammateId, false)).ownBrowser).toBeUndefined()
  })

  it('is a switch in the teammate dialog, cleared when turned off or the teammate is removed', async () => {
    const source = (path: string) => readFile(join(__dirname, path), 'utf8')
    const dialog = await source('../renderer/src/components/NewTeammateDialog.tsx')
    expect(dialog).toContain('aria-label="Own browser sign-ins"')
    const index = await source('index.ts')
    expect(index).toContain('if (teammate.ownBrowser !== true) await browserHost.forgetProfile(teammate.teammateId)')
    expect(index).toContain("if (typeof teammateId === 'string') await browserHost.forgetProfile(teammateId).catch(() => undefined)")
    expect(index).toContain('...(self.ownBrowser === true ? { browserProfile: { id: self.teammateId, name: self.name, hue: self.hue } } : {}),')
    expect(await source('codex-mission.ts')).toContain('...(peer?.browserProfile === undefined ? {} : { profile: peer.browserProfile }),')
  })
})
