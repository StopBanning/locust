import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { readConfirmedFolders, sameFolder, writeConfirmedFolders } from './workspace.js'

/*
 * A CONVERSATION IN A SETTINGS FOLDER IS ASKED ABOUT, ONCE (0.790).
 *
 * The picker has asked since 0.582, but a folder can be the window's without that question: picked again from the
 * folder menu, or chosen before 0.582. On 2026-10-18 three handoffs started in `.claude`, the window's folder, and
 * two of them ran there in Auto with nobody noticing. Now the first conversation in such a folder is not started
 * until the person says so, in the window (a card over the box, not a Windows dialog -- Colin: "maybe make it part
 * of the app?"), and the answer is kept, as an editor asks once whether to trust a folder.
 */

const HOME = 'C:\\Users\\person'
const roots: string[] = []
afterEach(async () => { for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true }) })
// The main process is not importable in a test; its handlers are read as text.
const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')

describe('the first conversation in a folder that is no place for teammates', () => {
  it('keeps the folders said yes to, and reads nothing odd as one', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-folder-cautions-'))
    roots.push(root)
    const file = join(root, 'folder-cautions.json')
    // The file only ever holds this machine's own folders, so they are spelled as the machine running the test spells them.
    const home = join(root, 'person')
    expect(readConfirmedFolders(file)).toEqual([])
    await writeConfirmedFolders(file, [join(home, '.claude')])
    expect(readConfirmedFolders(file)).toEqual([join(home, '.claude')])
    await writeFile(file, JSON.stringify({ schemaVersion: 1, confirmed: ['relative\\folder', 7, null, home] }), 'utf8')
    expect(readConfirmedFolders(file)).toEqual([home])
    await writeFile(file, 'not json', 'utf8')
    expect(readConfirmedFolders(file)).toEqual([])
  })

  it('matches the same folder only, as Windows spells it either way', () => {
    expect(sameFolder(`${HOME}\\.claude`, 'c:\\users\\PERSON\\.claude\\', 'win32')).toBe(true)
    expect(sameFolder(`${HOME}\\.claude`, `${HOME}\\.claude\\skills`, 'win32')).toBe(false)
    expect(sameFolder('/home/person/.claude', '/home/person/.Claude', 'linux')).toBe(false)
  })

  it('starts nothing for a new conversation there until the folder is said yes to, and asks in the window', () => {
    const start = main.slice(main.indexOf('ipcMain.handle(CODEX_MISSION_START_CHANNEL'), main.indexOf('const mcpTools = createLocustMcpTools'))
    expect(start).toContain("!('followUpOf' in request) && !('rewind' in request)")
    expect(start).toContain('!folderConfirmed(workspacePath) ? folderCaution(workspacePath)')
    expect(start).toContain("code: 'FOLDER_NOT_CONFIRMED'")
    expect(start).toContain('folderCaution: why')
    // Refused before anything starts, and with no Windows dialog: the window asks.
    expect(start.indexOf("code: 'FOLDER_NOT_CONFIRMED'")).toBeLessThan(start.indexOf('return startMission('))
    expect(start).not.toContain('showMessageBox')
  })

  it('keeps only the current folder, only from its own window, and only one that needed asking', () => {
    const confirm = main.slice(main.indexOf('ipcMain.handle(WORKSPACE_CONFIRM_CHANNEL'), main.indexOf('ipcMain.handle(WORKSPACE_CHOOSE_CHANNEL'))
    expect(confirm).toContain('if (!fromOwnWindow(event) || !workspaceChosen || folderCaution(workspacePath) === undefined) return false')
    expect(confirm).toContain('await confirmFolder(workspacePath)')
    // The folder chip is told why, whatever was answered.
    expect(main).toContain('workspaceCaution: folderCaution(workspacePath)!')
  })
})
