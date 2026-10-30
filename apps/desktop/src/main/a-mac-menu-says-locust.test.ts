import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { macMenuTemplate } from './mac-menu.js'

/**
 * A MAC'S MENU SAYS LOCUST (the first Mac run, 2026-10-30).
 *
 * With no menu of its own, a Mac had Electron's default, named for `app.name`: "About @teammate/desktop",
 * "Hide @teammate/desktop", "Quit @teammate/desktop". The menu is now Electron's default with Locust's name in it.
 */
describe("a Mac's application menu", () => {
  const menu = macMenuTemplate('Locust')
  const appMenu = menu[0]?.submenu as { role?: string; label?: string }[]

  it('is headed Locust, and its About, Hide and Quit say Locust', () => {
    expect(menu[0]?.label).toBe('Locust')
    expect(appMenu.filter((item) => item.label !== undefined).map((item) => [item.role, item.label])).toEqual([
      ['about', 'About Locust'],
      ['hide', 'Hide Locust'],
      ['quit', 'Quit Locust']
    ])
    expect(JSON.stringify(menu)).not.toContain('@teammate')
  })

  it("keeps everything else Electron's default gave: Services, Hide Others, Show All, Cmd+Q, and the Edit menu's copy and paste", () => {
    expect(appMenu.map((item) => item.role ?? 'separator')).toEqual(['about', 'separator', 'services', 'separator', 'hide', 'hideOthers', 'unhide', 'separator', 'quit'])
    expect(menu.slice(1).map((item) => item.role)).toEqual(['fileMenu', 'editMenu', 'viewMenu', 'windowMenu'])
  })

  it('is set on a Mac only, as the app becomes ready, and app.name is left alone', () => {
    const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')
    const ready = main.slice(main.indexOf('void app.whenReady().then(async () => {'), main.indexOf('const restored = await applyPendingRestore('))
    expect(ready).toContain("if (process.platform === 'darwin') Menu.setApplicationMenu(Menu.buildFromTemplate(macMenuTemplate('Locust')))")
    expect(main).not.toMatch(/app\.setName\(|app\.name\s*=/)
  })
})
