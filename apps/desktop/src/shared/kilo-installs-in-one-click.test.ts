import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { installCommand, runtimeInstallFacts } from './runtime-install.js'

/**
 * KILO INSTALLS IN ONE CLICK (0.838.x). Colin, 2026-10-28, typing npm's lines into a terminal to add Kilo: "we might
 * need to make this stuff easier for the user, like run the commands for them". Then: "now its saying not installed
 * it but its here". Kilo is one npm install, as Claude Code, Codex, Copilot and OpenCode are, and a "Get it" page
 * opened makes the next return to the window ask about every agent, the ones not installed too.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('Kilo installs in one click', () => {
  it('is one npm install of its CLI package, signed into only for a paid model', () => {
    expect(runtimeInstallFacts('kilo')?.install).toEqual({ kind: 'npm', packageName: '@kilocode/cli' })
    expect(installCommand('kilo')).toBe('npm install -g @kilocode/cli')
    expect(runtimeInstallFacts('kilo')?.signInOptional).toBe(true)
  })

  it('a Get it page opened asks about every agent on the next return, the uninstalled too', () => {
    const screen = read('../renderer/src/components/FirstLaunch.tsx')
    const getIt = screen.slice(screen.indexOf("openLink(vendorUrl(runtime.id) ?? '', setLinkRefusal)"), screen.indexOf('Get it ↗'))
    expect(getIt).toContain('window.dispatchEvent(new Event(INSTALL_PAGE_OPENED_EVENT))')
    const app = read('../renderer/src/App.tsx')
    const focus = app.slice(app.indexOf('const onFocus = (): void => {'), app.indexOf('if (signInOpened) {', app.indexOf('const onFocus = (): void => {')))
    // `true`: everything, the host's caches too, not only the installed agents still being checked.
    expect(focus).toContain('if (installPageOpened) {')
    expect(focus).toContain('askAgain(true)')
  })
})
