import { describe, expect, it } from 'vitest'

import { defaultAppMayOpen, fileMenuChoices, pathInsideFolder, relativeToFolder, rootedPath, systemPath } from './file-menu.js'

const folder = 'C:/work/locust'

describe('a right-click on a file path', () => {
  it('offers the editors that are installed, and the default app only for a file it shows rather than runs', () => {
    const labels = fileMenuChoices({ path: 'C:/work/locust/docs/plan.md', folder, platform: 'win32', editors: ['vscode'] }).map((choice) => choice.label)
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
    // A list of what may open, not of what may not: each of these runs when its default app opens it.
    for (const name of ['app.ts', 'page.hta', 'link.lnk', 'tool.jar', 'keys.reg', 'site.url', 'panel.cpl', 'run.py', 'run.sh', 'run.command', 'app.desktop', 'NOEXT']) {
      expect(defaultAppMayOpen(`C:/work/locust/${name}`)).toBe(false)
    }
    for (const name of ['notes.md', 'shot.PNG', 'report.pdf', 'index.html', 'clip.mp4']) {
      expect(defaultAppMayOpen(`C:/work/locust/${name}`)).toBe(true)
    }
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

describe('a copied path', () => {
  it('is written with the system separator, never a mix of both on Windows', () => {
    expect(systemPath(rootedPath('src/signup.ts', 'C:\\work\\app'), 'win32')).toBe('C:\\work\\app\\src\\signup.ts')
    expect(systemPath(relativeToFolder('C:/work/app/src/signup.ts', 'C:/work/app', 'win32'), 'win32')).toBe('src\\signup.ts')
    expect(systemPath('/work/app/src/signup.ts', 'linux')).toBe('/work/app/src/signup.ts')
  })
})
