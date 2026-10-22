import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { notATeammateFolder } from './workspace.js'

/*
 * A CHOSEN AGENT-SETTINGS FOLDER IS ASKED ABOUT (0.582), IN THE WINDOW (0.816).
 *
 * 0.572 stopped a LAUNCH from `.claude` becoming the workspace; a folder
 * picked in the window still had no word at all (handoff review finding d).
 * Picked on purpose it is the person's call -- Colin works in his `.claude`
 * on purpose. Until 0.816 the picker asked in a Windows message box; since
 * 0.790 the first new conversation in such a folder asks in the window, and
 * that is where it is asked now (Colin: "Locust asks in the window, never a
 * Windows dialog").
 */

const HOME = 'C:\\Users\\person'

describe('choosing a folder that is no place for teammates', () => {
  it('says which folder it is', () => {
    expect(notATeammateFolder(`${HOME}\\.claude`, HOME, 'win32')).toBe('.claude, where the agents keep their own settings')
  })

  it('asks about home too, and nothing about a project folder (control)', () => {
    expect(notATeammateFolder(HOME, HOME, 'win32')).toBe('your home folder')
    // Copilot CLI's own folder joined the guard in 0.587.
    expect(notATeammateFolder(`${HOME}\\.copilot\\session-state`, HOME, 'win32')).toBe('.copilot, where the agents keep their own settings')
    expect(notATeammateFolder(`${HOME}\\Documents\\shop`, HOME, 'win32')).toBeUndefined()
  })

  it('is not asked in a Windows dialog by the picker: the first conversation there asks, in the window', () => {
    // The main process is not importable in a test; its picker is read as text.
    const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')
    const picker = main.slice(main.indexOf('ipcMain.handle(WORKSPACE_CHOOSE_CHANNEL'), main.indexOf('A QUESTION ON THE SIDE'))
    expect(picker).not.toContain('showMessageBox')
    expect(picker).toContain('switchFolder(next)')
    // The question in the window: a new conversation in such a folder, not yet said yes to, does not start.
    const start = main.slice(main.indexOf('ipcMain.handle(CODEX_MISSION_START_CHANNEL'))
    expect(start).toMatch(/folderCaution\(workspacePath\)/)
    expect(start).toContain("code: 'FOLDER_NOT_CONFIRMED'")
  })
})
