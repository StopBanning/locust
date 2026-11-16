import { readFileSync, statSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, it } from 'vitest'

import { SETTINGS_A_BROWSER_MAY_CHANGE } from '../shared/ipc.js'
import { createTeammateStore } from './teammate-store.js'
import { settingsChangeKept } from './workspace-settings-change.js'

/**
 * THE CODE FONT CAN BE PAPER MONO (0.877).
 *
 * Colin, 2026-11-14, with Geist Mono and Paper Mono set side by side: "we can have it as a font option, I love that
 * idea". An option, never a replacement: Geist Mono stays the default, and only the code face changes. The Rust app
 * reads the same `codeFont` key, so one choice holds in both.
 */

const SRC = fileURLToPath(new URL('../renderer/src/', import.meta.url))
const tokens = readFileSync(join(SRC, 'tokens.css'), 'utf8')
const app = readFileSync(join(SRC, 'App.tsx'), 'utf8')
const screens = readFileSync(join(SRC, 'components', 'Screens.tsx'), 'utf8')

const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})
async function store() {
  const root = await mkdtemp(join(tmpdir(), 'locust-codefont-'))
  roots.push(root)
  return { root, store: createTeammateStore({ rootDirectory: root }) }
}

describe('the stored choice', () => {
  it('is Geist Mono until Paper Mono is picked, and is kept once it is', async () => {
    const { root, store: teammates } = await store()
    expect((await teammates.readSettings()).codeFont).toBe('geist')
    await teammates.writeSettings({ codeFont: 'paper' })
    expect((await createTeammateStore({ rootDirectory: root }).readSettings()).codeFont).toBe('paper')
    await teammates.writeSettings({ codeFont: 'geist' })
    expect((await teammates.readSettings()).codeFont).toBe('geist')
  })

  it('reads anything but paper as Geist Mono', async () => {
    const { store: teammates } = await store()
    for (const value of ['Paper', 'comic', '', 1, true, null, {}]) {
      await teammates.writeSettings({ codeFont: value })
      expect((await teammates.readSettings()).codeFont, String(value)).toBe('geist')
    }
  })

  it('survives a write that leaves it out', async () => {
    const { store: teammates } = await store()
    await teammates.writeSettings({ codeFont: 'paper' })
    await teammates.writeSettings({ replySize: 'large' })
    expect((await teammates.readSettings()).codeFont).toBe('paper')
  })

  it('may be changed from a browser, like the reply size: it grants nothing', () => {
    expect(SETTINGS_A_BROWSER_MAY_CHANGE).toContain('codeFont')
    expect(settingsChangeKept({ codeFont: 'paper', autoMode: true })).toEqual({ codeFont: 'paper' })
  })
})

describe('the face', () => {
  it('is bundled: a real TrueType file and its licence on disk, every weight in one file', () => {
    const font = join(SRC, 'assets', 'fonts', 'PaperMono-Variable.ttf')
    expect(statSync(font).size).toBeGreaterThan(100_000)
    expect([...readFileSync(font).subarray(0, 4)]).toEqual([0, 1, 0, 0])
    expect(readFileSync(join(SRC, 'assets', 'fonts', 'PAPER-MONO-OFL.txt'), 'utf8')).toContain('SIL OPEN FONT LICENSE')
    const face = tokens.slice(tokens.indexOf("font-family: 'Paper Mono'"))
    expect(face.slice(0, 200)).toContain("url('./assets/fonts/PaperMono-Variable.ttf') format('truetype')")
    expect(face.slice(0, 200)).toContain('font-weight: 100 800')
  })

  it('changes only the code token, with Geist Mono behind it', () => {
    const rule = tokens.slice(tokens.indexOf(":root[data-codefont='paper'] {"))
    const body = rule.slice(0, rule.indexOf('}'))
    expect(body).toContain("--lc-font-mono: 'Paper Mono', 'Geist Mono', ui-monospace")
    expect(body).not.toContain('--lc-font-ui')
    expect(body).not.toContain('--lc-font-prose')
    // The default token is untouched.
    expect(tokens).toContain("  --lc-font-mono: 'Geist Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;")
  })
})

describe('the window', () => {
  it('writes the choice on the root, read at start and whenever it is looked at again', () => {
    expect(app).toContain('document.documentElement.dataset.codefont = codeFont')
    expect(app.match(/setCodeFont\(settings\.codeFont === 'paper' \? 'paper' : 'geist'\)/g)?.length).toBe(3)
  })

  it('sends only the key it changes, and puts the old choice back if the write fails', () => {
    expect(app).toContain('?.changeWorkspaceSettings({ codeFont: next })')
    expect(app).toContain('.catch(() => setCodeFont(before))')
  })

  it('offers the two in Settings, Appearance, each set in its own face', () => {
    const section = screens.slice(screens.indexOf('data-setting="code-font"'))
    expect(section.slice(0, 2000)).toContain('<h2 className="lc-settings__heading">Code font</h2>')
    expect(section.slice(0, 2000)).toContain("['geist', 'Geist Mono']")
    expect(section.slice(0, 2000)).toContain("['paper', 'Paper Mono']")
    expect(section.slice(0, 2000)).toContain('aria-checked={codeFont === option}')
    expect(section.slice(0, 2000)).toContain('onClick={() => onCodeFontChange(option)}')
    const shell = readFileSync(join(SRC, 'shell.css'), 'utf8')
    expect(shell).toContain(".lc-codefont-option--paper { font-family: 'Paper Mono'")
  })
})
