import { describe, expect, it } from 'vitest'

import { fileMenuChoices, pathInsideFolder, relativeToFolder, rootedPath } from './file-menu.js'

const folder = 'C:/work/locust'

describe('a right-click on a file path', () => {
  it('offers the editors that are installed, and not a script to the default app', () => {
    const labels = fileMenuChoices({ path: 'C:/work/locust/src/app.ts', folder, platform: 'win32', editors: ['vscode'] }).map((choice) => choice.label)
    expect(labels).toEqual([
      'Add to my next message',
      'Open in VS Code',
      'Open in the default app',
      'Show in Explorer',
      'Copy path',
      'Copy relative path',
      'Copy file name'
    ])
    const script = fileMenuChoices({ path: 'C:/work/locust/tools/run.ps1', folder, platform: 'win32', editors: ['cursor'] }).map((choice) => choice.id)
    expect(script).not.toContain('default')
    expect(script).toContain('cursor')
  })

  it('offers Copy only when the path is outside the conversation folder', () => {
    expect(fileMenuChoices({ path: 'D:/other/notes.md', folder, platform: 'win32', editors: ['vscode', 'cursor'] }).map((choice) => choice.id)).toEqual([
      'copy-path',
      'copy-relative',
      'copy-name'
    ])
    expect(pathInsideFolder('C:/work/locust/../secret.md', folder, 'win32')).toBe(false)
    expect(relativeToFolder('C:/work/locust/src/app.ts', folder, 'win32')).toBe('src/app.ts')
    const named = rootedPath('README.md', folder)
    expect(named).toBe('C:/work/locust/README.md')
    expect(fileMenuChoices({ path: named, folder, platform: 'win32', editors: [] }).map((choice) => choice.id)).toContain('attach')
    expect(fileMenuChoices({ path: rootedPath('../secret.md', folder), folder, platform: 'win32', editors: ['vscode'] }).map((choice) => choice.id)).toEqual([
      'copy-path',
      'copy-relative',
      'copy-name'
    ])
  })

  it('names the file manager for the system', () => {
    expect(fileMenuChoices({ path: '/work/locust/a.ts', folder: '/work/locust', platform: 'darwin', editors: [] }).some((choice) => choice.label === 'Show in Finder')).toBe(true)
    expect(fileMenuChoices({ path: '/work/locust/a.ts', folder: '/work/locust', platform: 'linux', editors: [] }).some((choice) => choice.label === 'Show in the file manager')).toBe(true)
  })
})
