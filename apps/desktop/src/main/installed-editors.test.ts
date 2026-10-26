import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { delimiter, join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { editorSpawn, editorsOnPath } from './installed-editors.js'

describe('editors are taken from PATH', () => {
  it('lists one that is on PATH and skips one that is not, without guessing an install folder', () => {
    const bin = 'C:/tools'
    const there = new Set([join(bin, 'code.cmd'), join(bin, 'cursor.exe')])
    const found = editorsOnPath('win32', { PATH: ['C:/empty', bin].join(delimiter) }, (file) => there.has(file))
    expect(found).toEqual([
      { id: 'vscode', command: join(bin, 'code.cmd') },
      { id: 'cursor', command: join(bin, 'cursor.exe') }
    ])
    expect(editorsOnPath('win32', { PATH: 'C:/empty' }, () => false)).toEqual([])
    expect(editorsOnPath('linux', { PATH: '/usr/bin' }, (file) => file === '/usr/bin/code')).toEqual([{ id: 'vscode', command: '/usr/bin/code' }])
  })

  it('keeps a file as an argument, and runs a Windows command shim through System32 cmd.exe, verbatim', () => {
    const shim = editorSpawn('C:/tools/code.cmd', 'C:/work/app.ts', 'win32', { SystemRoot: 'C:\\Windows' })
    expect(shim.file).toBe('C:\\Windows\\System32\\cmd.exe')
    expect(shim.windowsVerbatimArguments).toBe(true)
    expect(shim.args).toHaveLength(1)
    expect(editorSpawn('/usr/bin/code', '/work/app.ts', 'linux', {})).toEqual({ file: '/usr/bin/code', args: ['/work/app.ts'], windowsVerbatimArguments: false })
    expect(editorSpawn('C:/tools/cursor.exe', 'C:/work/a&b.ts', 'win32', {})).toEqual({ file: 'C:/tools/cursor.exe', args: ['C:/work/a&b.ts'], windowsVerbatimArguments: false })
  })

  /*
   * MEASURED with real cmd.exe (0.836 review): the first cut spawned the shim with `shell: true`, which escapes
   * nothing, so a teammate's file named `a&mkdir pwned&b.txt` made "Open in VS Code" run `mkdir pwned`.
   */
  it.runIf(process.platform === 'win32')('passes a file named with & to the shim whole, and runs nothing else', () => {
    const folder = mkdtempSync(join(tmpdir(), 'locust-editor-shim-'))
    const shim = join(folder, 'code.cmd')
    // As VS Code's and Cursor's shims do, the argument goes on still quoted (`%*` there), so its `&` stays text.
    writeFileSync(shim, '@echo off\r\necho %1>"%~dp0got.txt"\r\n', 'utf8')
    const file = join(folder, 'a&mkdir pwned&b.txt')
    writeFileSync(file, 'x', 'utf8')
    const launch = editorSpawn(shim, file, 'win32', process.env)
    const result = spawnSync(launch.file, [...launch.args], { cwd: folder, windowsVerbatimArguments: launch.windowsVerbatimArguments, windowsHide: true })
    expect(result.status).toBe(0)
    expect(existsSync(join(folder, 'pwned'))).toBe(false)
    expect(readFileSync(join(folder, 'got.txt'), 'utf8').trim()).toBe(`"${file}"`)
  })
})
