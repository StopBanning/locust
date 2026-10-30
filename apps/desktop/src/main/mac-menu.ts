import type { MenuItemConstructorOptions } from 'electron'

/**
 * A MAC'S APPLICATION MENU, NAMED FOR LOCUST.
 *
 * Locust set no menu, so a Mac had Electron's default, which names its items
 * after `app.name`: the first Mac run (2026-10-30) read "About @teammate/desktop",
 * "Hide @teammate/desktop" and "Quit @teammate/desktop", the package's name.
 * `app.name` itself stays as it is: the profile folder and the keychain's
 * "Safe Storage" item are named for it, and renaming it would leave the
 * person's profile and saved keys behind. So this is Electron's default menu,
 * item for item, with Locust's name where the default puts the package's.
 */
export function macMenuTemplate(name: string): MenuItemConstructorOptions[] {
  return [
    {
      label: name,
      submenu: [
        { role: 'about', label: `About ${name}` },
        { type: 'separator' },
        { role: 'services' },
        { type: 'separator' },
        { role: 'hide', label: `Hide ${name}` },
        { role: 'hideOthers' },
        { role: 'unhide' },
        { type: 'separator' },
        { role: 'quit', label: `Quit ${name}` }
      ]
    },
    { role: 'fileMenu' },
    { role: 'editMenu' },
    { role: 'viewMenu' },
    { role: 'windowMenu' }
  ]
}
