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

  it('keeps a file as an argument, and shells only a Windows command shim', () => {
    expect(editorSpawn('C:/tools/code.cmd', 'C:/work/app.ts', 'win32')).toEqual({ file: 'C:/tools/code.cmd', args: ['C:/work/app.ts'], shell: true })
    expect(editorSpawn('/usr/bin/code', '/work/app.ts', 'linux').shell).toBe(false)
  })
})
